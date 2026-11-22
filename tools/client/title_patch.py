"""Build 39167 hooks for the server-delivered text used by patch-gw.py."""

import hashlib
import os
from pathlib import Path
import shutil
import struct
import subprocess
import tempfile

import pefile


STOCK_SHA256 = "3aa5dcde58da08230c7fbff03320086cfd422c63a025822020aea3ecd1d511c4"
DLL_NAME = "KrytaTitles.dll"
EXPORTS = (b"kryta_title_name", b"kryta_title_hover_v2")
RANK_CALL, RANK_WORKER = 0x51E7E0, 0x4130B0
HOVER_CALL, HOVER_FORMATTER, HOVER_RETURN = 0x4A88DB, 0x3C76D0, 0x4A891F
METADATA_LOOKUP = 0x1A7540
METADATA_PROLOGUE = bytes.fromhex("55 8b ec 56 8b 75 08")


def align(size, boundary):
    return (size + boundary - 1) // boundary * boundary


def branch(opcode, source, destination):
    return bytes([opcode]) + struct.pack("<i", destination - source - 5)


def hook(rva, body, prologue=b""):
    # The call thunk returns the relocated address. Subtract its RVA for the image base.
    call = rva + len(prologue)
    return (prologue + branch(0xE8, call, call + 10 + len(body))
            + b"\x2d" + struct.pack("<I", call + 5) + body + b"\x8b\x04\x24\xc3")


def callback(url_rva, iat_rva, argument_count):
    # lea edx,[eax+url]; push edx; call [eax+iat]; add esp,argument bytes.
    return (b"\x8d\x90" + struct.pack("<I", url_rva) + b"\x52\xff\x90"
            + struct.pack("<I", iat_rva) + bytes([0x83, 0xC4, argument_count * 4]))


def rank_hook(rva, url, iat):
    prologue = bytes.fromhex("55 89 e5")  # preserve the stock cdecl stack
    body = bytes.fromhex("ff 75 10 ff 75 08") + callback(url, iat, 3)
    body += bytes.fromhex("85 c0 74 03 89 45 10 c9")  # substitute only on hit; leave
    body += branch(0xE9, rva + len(prologue) + 10 + len(body), RANK_WORKER)
    return hook(rva, body, prologue)


def hover_hook(rva, url, iat):
    # Goal, next rank, current rank, title ID. Stock formatter args remain on the stack.
    body = bytes.fromhex("ff 77 18 ff 77 14 ff 77 08 8b 55 fc ff 72 04")
    body += callback(url, iat, 5)
    body += bytes.fromhex("85 c0 74 08 83 c4 18")  # on hit discard the six stock args
    body += branch(0xE9, rva + 10 + len(body), HOVER_RETURN)
    body += branch(0xE8, rva + 10 + len(body), HOVER_FORMATTER)
    body += branch(0xE9, rva + 10 + len(body), HOVER_CALL + 5)
    return hook(rva, body)


def metadata_hook(rva, custom_metadata):
    # Build 39167's row and hover consumers only read the fallback text ID
    # from +8. Earned custom rows get their real name from the rank record.
    prefix = bytes.fromhex("83 7c 24 04 30")  # cmp dword [esp+4],48
    # Locate the fallback record relative to this thunk's return address.
    custom_ret = b"\xe8\0\0\0\0\x58\x05" + struct.pack(
        "<i", custom_metadata - (rva + len(prefix) + 2 + 5)
    ) + b"\xc3"
    stock = METADATA_PROLOGUE
    stock += branch(0xE9, rva + len(prefix) + 2 + len(custom_ret) + len(stock),
                    METADATA_LOOKUP + len(METADATA_PROLOGUE))
    return prefix + bytes([0x72, len(custom_ret)]) + custom_ret + stock


def imports(pe, rva):
    descriptors = b"".join(entry.struct.__pack__() for entry in pe.DIRECTORY_ENTRY_IMPORT)
    data = bytearray(descriptors + bytes(40))  # runtime descriptor and null descriptor

    def append(blob):
        address = rva + len(data)
        data.extend(blob)
        return address

    name = append(DLL_NAME.encode("ascii") + b"\0")
    symbols = []
    for export in EXPORTS:
        data.extend(bytes(-len(data) % 2))
        symbols.append(append(b"\0\0" + export + b"\0"))
    data.extend(bytes(-len(data) % 4))
    thunks = struct.pack("<III", *symbols, 0)
    lookup, iat = append(thunks), append(thunks)
    struct.pack_into("<IIIII", data, len(descriptors), lookup, 0, 0, name, iat)
    return len(descriptors) + 40, iat, data


def patch_titles(stock, source, url):
    if hashlib.sha256(stock).hexdigest() != STOCK_SHA256:
        raise ValueError("custom titles require the measured build 39167 Gw.exe")
    pe = pefile.PE(data=source)
    optional = pe.OPTIONAL_HEADER
    security = optional.DATA_DIRECTORY[4]
    raw_start = pe.get_overlay_data_start_offset()
    if raw_start != security.VirtualAddress or raw_start + security.Size != len(source):
        raise ValueError("unexpected data after the client sections")
    headers = pe.sections[-1].get_file_offset() + 40
    if headers + 80 > optional.SizeOfHeaders or any(source[headers:headers + 80]):
        raise ValueError("no room for runtime title section headers")
    if raw_start % optional.FileAlignment:
        raise ValueError("unexpected client section alignment")

    data_rva = optional.SizeOfImage
    import_size, iat, data = imports(pe, data_rva)
    data.extend(bytes(-len(data) % 2))
    url_rva = data_rva + len(data)
    data.extend((url + "\0").encode("utf-16le"))
    data.extend(bytes(-len(data) % 4))
    custom_metadata = data_rva + len(data)
    data.extend(struct.pack("<III", 0, 0, 1))  # stock null text fallback
    code_rva = align(data_rva + len(data), optional.SectionAlignment)
    code = rank_hook(code_rva, url_rva, iat)
    hover_rva = code_rva + len(code)
    code += hover_hook(hover_rva, url_rva, iat + 4)
    metadata_rva = code_rva + len(code)
    code += metadata_hook(metadata_rva, custom_metadata)

    image = bytearray(source[:raw_start])
    for site, original, opcode, destination in (
        (RANK_CALL, RANK_WORKER, 0xE8, code_rva),
        (HOVER_CALL, HOVER_FORMATTER, 0xE9, hover_rva),
    ):
        offset = pe.get_offset_from_rva(site)
        if image[offset:offset + 5] != branch(0xE8, site, original):
            raise ValueError(f"unexpected title instruction at RVA {site:#x}")
        image[offset:offset + 5] = branch(opcode, site, destination)
    offset = pe.get_offset_from_rva(METADATA_LOOKUP)
    end = offset + len(METADATA_PROLOGUE)
    if image[offset:end] != METADATA_PROLOGUE:
        raise ValueError(f"unexpected title metadata instruction at RVA {METADATA_LOOKUP:#x}")
    image[offset:end] = branch(0xE9, METADATA_LOOKUP, metadata_rva) + b"\x90\x90"
    for offset, name, rva, payload, flags in (
        (0, b".titdata", data_rva, data, 0xC0000040),
        (40, b".title", code_rva, code, 0x60000020),
    ):
        raw_size = align(len(payload), optional.FileAlignment)
        struct.pack_into("<8sIIIIIIHHI", image, headers + offset, name,
                         len(payload), rva, raw_size, len(image), 0, 0, 0, 0, flags)
        image.extend(payload)
        image.extend(bytes(raw_size - len(payload)))
    struct.pack_into("<II", image, optional.DATA_DIRECTORY[1].get_file_offset(), data_rva, import_size)
    struct.pack_into("<II", image, security.get_file_offset(), 0, 0)
    struct.pack_into("<H", image, pe.FILE_HEADER.get_field_absolute_offset("NumberOfSections"),
                     pe.FILE_HEADER.NumberOfSections + 2)
    for field, value in (
        ("SizeOfImage", align(code_rva + len(code), optional.SectionAlignment)),
        ("SizeOfCode", optional.SizeOfCode + align(len(code), optional.FileAlignment)),
        ("SizeOfInitializedData", optional.SizeOfInitializedData + align(len(data), optional.FileAlignment)),
        ("CheckSum", 0),
    ):
        struct.pack_into("<I", image, optional.get_field_absolute_offset(field), value)
    return image


def build_runtime():
    where = Path(os.environ["ProgramFiles(x86)"]) / "Microsoft Visual Studio/Installer/vswhere.exe"
    location = subprocess.check_output(
        [str(where), "-latest", "-products", "*", "-requires",
         "Microsoft.VisualStudio.Component.VC.Tools.x86.x64", "-property", "installationPath"],
        text=True,
    ).strip()
    if not location:
        raise ValueError("Visual Studio x86 C++ tools are required to build the title runtime")
    env = os.environ.copy()
    env["KRYTA_VCVARS"] = str(Path(location) / "VC/Auxiliary/Build/vcvars32.bat")
    variables = subprocess.check_output(
        'cmd.exe /d /s /c "call "%KRYTA_VCVARS%" >nul && set"', env=env, text=True,
    )
    env.update(line.split("=", 1) for line in variables.splitlines() if "=" in line and not line.startswith("="))
    include = Path(os.environ.get("KRYTA_TITLE_JSON_INCLUDE",
                                  str(Path.home() / "Py4GW_Reforged_Native/third_party")))
    if not (include / "nlohmann/json.hpp").is_file():
        raise ValueError("set KRYTA_TITLE_JSON_INCLUDE to the directory containing nlohmann/json.hpp")
    compiler = shutil.which("cl.exe", path=env.get("Path", env.get("PATH")))
    if not compiler:
        raise ValueError("the x86 Visual Studio environment did not provide cl.exe")
    source = Path(__file__).with_name("title-runtime.cpp").resolve()
    with tempfile.TemporaryDirectory(prefix="kryta-title-runtime-") as temp:
        subprocess.run([compiler, "/nologo", "/LD", "/MT", "/EHsc", "/std:c++17", "/O2",
                        f"/I{include}", str(source), f"/Fe:{DLL_NAME}", "/link",
                        "Winhttp.lib", "Shell32.lib"], cwd=temp, env=env, check=True)
        return (Path(temp) / DLL_NAME).read_bytes()


def validate_runtime(image):
    with pefile.PE(data=image) as pe:
        directory = getattr(pe, "DIRECTORY_ENTRY_EXPORT", None)
        exports = {symbol.name for symbol in directory.symbols} if directory else set()
        if pe.FILE_HEADER.Machine != 0x14C or not pe.is_dll() or not set(EXPORTS) <= exports:
            raise ValueError("expected an x86 KrytaTitles.dll that supports title rows from 48")

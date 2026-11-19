"""One capture file, read in the layer it declares: the rules the reader and the comparison share.

A capture is written in one of two layers and the two are not interchangeable (``docs/port/02-wire.md``,
"Three kinds of capture"):

  * ``struct`` - what ``tools/client/sniff-capture.py`` records, because it hooks the client's own
    receive and send paths. A record holds the client's message struct: a **four-byte** header (the
    u16 id and a zero u16), every field widened to four bytes, and zero padding out to the struct's
    size.
  * ``wire`` - what ``worldserver`` writes when ``CaptureFile`` names a file. A record holds the
    packed frame: a **two-byte** header and the fields alone.

The layer travels in the file's own header record. This module is the one place that turns a record
into (header, payload), and the one place that refuses a file which does not say what it is.
"""

import json
import sys

#: The header length each layer carries, which is the only difference a reader may act on.
LAYERS = {"struct": 4, "wire": 2}

#: Which layer a tool writes. The tool that made the file is the evidence for its layer, so a capture
#: from before the layer field existed still reads correctly and without a size guess.
TOOL_LAYER = {
    "worldserver": "wire",
    "tools/client/sniff-capture.py": "struct",
}


def describe(handle) -> dict:
    """The capture's own header record, with its layer checked."""
    line = handle.readline()
    try:
        head = json.loads(line)
    except json.JSONDecodeError:
        raise SystemExit(f"first line is not a header record: {line[:80]!r}")
    if head.get("record") != "capture":
        raise SystemExit(f"first line is not a header record: {line[:80]!r}")
    return head


def layer_of(head: dict, assume: str | None) -> tuple[str, int]:
    """The layer and header length: the file's own field, else its tool's, else an explicit assumption."""
    layer = head.get("layer") or TOOL_LAYER.get(str(head.get("tool") or ""))
    length = head.get("header_len")
    if layer in LAYERS:
        if length is not None and int(length) != LAYERS[layer]:
            raise SystemExit(
                f"the file says layer {layer} with header_len {length}; that layer is {LAYERS[layer]}"
            )
        return layer, LAYERS[layer]
    if assume is not None:
        print(
            f"--- note: no layer in the file, assuming {assume} ({LAYERS[assume]} bytes)",
            file=sys.stderr,
        )
        return assume, LAYERS[assume]
    raise SystemExit(
        "this capture does not state its layer; pass --assume-layer struct|wire. A six-byte 0x017C is "
        "wire, a twenty-byte one is the client's struct (docs/port/02-wire.md, Three kinds of capture)"
    )


def messages(path: str, assume: str | None):
    """Yield (seq, direction, header, size, payload_bytes) for every message record."""
    with open(path, encoding="utf-8") as handle:
        head = describe(handle)
        layer, header_len = layer_of(head, assume)
        for line in handle:
            if not line.startswith('{"seq"'):
                continue
            record = json.loads(line)
            data = bytes.fromhex(record.get("data") or "")
            yield record.get("seq"), record.get("direction"), int(record["header"]), len(data), data[header_len:]


def layer_of_path(path: str, assume: str | None) -> tuple[str, int]:
    """The (layer, header length) a whole file declares."""
    with open(path, encoding="utf-8") as handle:
        return layer_of(describe(handle), assume)


def self_test() -> int:
    """No file, no client: the layer rule and the offset that follows from it."""
    assert LAYERS["struct"] == 4 and LAYERS["wire"] == 2

    head_wire = {"record": "capture", "layer": "wire", "header_len": 2}
    head_struct = {"record": "capture", "layer": "struct", "header_len": 4}
    assert layer_of(head_wire, None) == ("wire", 2)
    assert layer_of(head_struct, None) == ("struct", 4)
    try:
        layer_of({"record": "capture"}, None)
        raise AssertionError("a file without a layer must be refused")
    except SystemExit:
        pass
    assert layer_of({"record": "capture"}, "struct") == ("struct", 4)

    # The same message in both layers: 0x017C is six bytes at the wire and twenty through the hook. Both
    # carry the same two ids; the wire packs each to two bytes and the struct widens each to four, which
    # is why a reader must take the payload from the declared header length and never compare raw records.
    wire = bytes.fromhex("7c013f3f0000")
    struct = bytes.fromhex("7c0100003f0000003f0000000000000000000000")
    assert len(wire) == 6 and len(struct) == 20
    assert wire[2:] == bytes.fromhex("3f3f0000"), "wire payload: two u16 ids"
    assert struct[4:] == bytes.fromhex("3f0000003f0000000000000000000000"), "struct payload: two u32 ids"

    assert TOOL_LAYER["worldserver"] == "wire"
    assert TOOL_LAYER["tools/client/sniff-capture.py"] == "struct"
    assert layer_of({"record": "capture", "tool": "worldserver"}, None) == ("wire", 2), \
        "an older capture reads through its tool"
    print("self-test: the layer rule, its refusal, and the two-layer offset all behave")
    return 0

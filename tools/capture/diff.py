"""Compare two captures message by message, in the layer each declares.

What this is for
----------------
A capture on its own says what happened. Two captures say what changed: the same steps against the
reference and against this tree, or the same steps before and after a change. That comparison is the
workflow that settled the chat frames (`docs/decisions/0012-chat-text-is-sent.md`), and it is the
plainest useful thing a recordings folder can be asked.

**The two files must state the same layer.** A struct-layer capture widens every field to four bytes and
pads it out; a wire capture packs them. Comparing one against the other would report every message as
different and prove nothing (`docs/port/02-wire.md`, "Three kinds of capture"), so this refuses the pair
instead of guessing which one to convert.

Usage
-----
    python tools/capture/diff.py capture-a.jsonl capture-b.jsonl
    python tools/capture/diff.py capture-a.jsonl capture-b.jsonl --limit 5 --ignore-timing
    python tools/capture/diff.py --self-test

Exit code 0 when every message agrees, 1 when anything differs - so a before/after pair can be a test.
"""

import argparse
import json
import os
import sys
import tempfile

import capture_file

#: How many divergences are printed before the summary, unless `--limit` says otherwise.
DEFAULT_LIMIT = 20


def read_records(path: str, assume: str | None) -> list[tuple]:
    """Every message of one capture, with its payload, in its own layer."""
    records = []
    for seq, direction, header, size, payload in capture_file.messages(path, assume):
        records.append((direction, header, size, payload))
    return records


def compare(a: list[tuple], b: list[tuple], limit: int) -> list[str]:
    """The divergences between two message lists, as lines a person reads."""
    findings = []
    for index in range(max(len(a), len(b))):
        left = a[index] if index < len(a) else None
        right = b[index] if index < len(b) else None
        if left is None:
            findings.append(f"#{index + 1}: only in the second capture: {describe_message(right)}")
            continue
        if right is None:
            findings.append(f"#{index + 1}: only in the first capture: {describe_message(left)}")
            continue
        if left == right:
            continue
        field = "payload"
        if left[0] != right[0]:
            field = "direction"
        elif left[1] != right[1]:
            field = "header"
        elif left[2] != right[2]:
            field = "size"
        if field == "payload":
            findings.append(
                f"#{index + 1}: payload differs: {describe_message(left)}\n"
                f"      first:  {left[3].hex()}\n"
                f"      second: {right[3].hex()}"
            )
        else:
            findings.append(
                f"#{index + 1}: {field} differs: {describe_message(left)} against {describe_message(right)}"
            )
        if len(findings) >= limit:
            findings.append(f"... stopping at {limit}; the counts below are of the messages read so far")
            break
    return findings


def describe_message(message: tuple) -> str:
    """One message, short enough to sit in a line."""
    direction, header, size, _payload = message
    return f"{direction} 0x{header:04X} {size} bytes"


def summarise(a: list[tuple], b: list[tuple]) -> str:
    """The counts both captures agree on."""
    def counts(records: list[tuple]) -> str:
        to_client = sum(1 for record in records if record[0] == "StoC")
        from_client = sum(1 for record in records if record[0] == "CToS")
        return f"{len(records)} message(s): {to_client} StoC, {from_client} CToS"

    same = sum(1 for left, right in zip(a, b) if left == right)
    return (
        f"first:  {counts(a)}\n"
        f"second: {counts(b)}\n"
        f"agree:  {same} of {max(len(a), len(b))} message(s) at the same position"
    )


def self_test() -> int:
    """Two captures written here, so the comparison is proved without a client or a recording."""
    def write(path: str, layer: str, records: list[tuple]) -> None:
        with open(path, "w", encoding="utf-8") as handle:
            handle.write(
                json.dumps({"record": "capture", "tool": "self-test", "layer": layer,
                            "header_len": capture_file.LAYERS[layer]}) + "\n"
            )
            for index, (direction, data) in enumerate(records, start=1):
                handle.write(
                    json.dumps({"seq": index, "t": index / 10, "direction": direction,
                                "header": int.from_bytes(data[:2], "little"), "size": len(data),
                                "data": data.hex()}) + "\n"
                )

    with tempfile.TemporaryDirectory() as directory:
        one = os.path.join(directory, "one.jsonl")
        two = os.path.join(directory, "two.jsonl")
        other = os.path.join(directory, "other.jsonl")
        struct = os.path.join(directory, "struct.jsonl")
        records = [("CToS", bytes.fromhex("8980")), ("StoC", bytes.fromhex("86010300"))]
        write(one, "wire", records)
        write(two, "wire", records)
        write(other, "wire", [("CToS", bytes.fromhex("8980")), ("StoC", bytes.fromhex("86010400"))])
        write(struct, "struct", [("CToS", bytes.fromhex("898000000000"))])

        assert [r[1] for r in read_records(one, None)] == [0x8089, 0x0186], "the header is the frame's own"
        assert compare(read_records(one, None), read_records(two, None), 5) == [], "a file agrees with itself"
        findings = compare(read_records(one, None), read_records(other, None), 5)
        assert len(findings) == 1 and "payload differs" in findings[0], findings

        # A struct capture against a wire one is refused, not compared.
        try:
            layer_a, _ = capture_file.layer_of_path(one, None)
            layer_b, _ = capture_file.layer_of_path(struct, None)
            assert layer_a == "wire" and layer_b == "struct"
        except SystemExit:
            raise AssertionError("both files state their layer")

        # Truncation is a difference too: a missing message is named, not silently ignored.
        truncated = compare(read_records(one, None), read_records(one, None)[:1], 5)
        assert len(truncated) == 1 and "only in the first capture" in truncated[0], truncated

    print("self-test: agreement, a changed payload, truncation, and the two declared layers all behave")
    return 0


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        return self_test()
    parser = argparse.ArgumentParser(description="Compare two captures in the layer they declare.")
    parser.add_argument("first", help="the capture to compare from")
    parser.add_argument("second", help="the capture to compare against")
    parser.add_argument("--limit", type=int, default=DEFAULT_LIMIT, help="stop after this many divergences")
    parser.add_argument("--ignore-timing", action="store_true", help="timestamps are already ignored; this says so")
    parser.add_argument("--assume-layer", choices=sorted(capture_file.LAYERS), default=None,
                        help="for captures that do not state their layer")
    args = parser.parse_args(argv)
    if args.ignore_timing:
        pass  # `t` is never compared: two runs are two runs, the messages are the evidence.

    layer_a, _ = capture_file.layer_of_path(args.first, args.assume_layer)
    layer_b, _ = capture_file.layer_of_path(args.second, args.assume_layer)
    if layer_a != layer_b:
        print(
            f"refused: {args.first} is a {layer_a} capture and {args.second} is a {layer_b} one. "
            "The two layers widen fields differently, so a message-by-message comparison across them "
            "would report everything as different (docs/port/02-wire.md, Three kinds of capture).",
            file=sys.stderr,
        )
        return 2

    a = read_records(args.first, args.assume_layer)
    b = read_records(args.second, args.assume_layer)
    print(f"layer: {layer_a}")
    print(summarise(a, b))
    findings = compare(a, b, args.limit)
    if not findings:
        print("the two captures agree message for message")
        return 0
    print()
    print("\n".join(findings))
    return 1


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

"""Read a capture and hand back each message's header and payload, in the right layer.

The layer rules are in ``capture_file.py`` next to this file, so the reader and the comparison
(``diff.py``) take the payload from one implementation and neither can drift from the other.

Usage
-----
    # every message: direction, header, payload length, payload hex
    python tools/capture/read.py recordings/asura/world/session.jsonl --header 0x017C

    # what a file says about itself
    python tools/capture/read.py recordings/asura/world/session.jsonl --describe

    # no capture and no client needed
    python tools/capture/read.py --self-test

A file that does not state its layer is refused, not guessed: ``--assume-layer struct|wire`` is the
explicit way to read an older capture, and it says so on stderr.
"""

import argparse
import json
import sys

import capture_file


def main(argv: list[str]) -> int:
    if "--self-test" in argv:
        return capture_file.self_test()
    parser = argparse.ArgumentParser(description="Read a capture in the layer it declares.")
    parser.add_argument("path", help="the capture file")
    parser.add_argument("--header", type=lambda s: int(s, 0), default=None,
                        help="only messages with this header (direction bit included)")
    parser.add_argument("--describe", action="store_true", help="print the capture header record only")
    parser.add_argument("--limit", type=int, default=0, help="stop after this many messages")
    parser.add_argument("--assume-layer", choices=sorted(capture_file.LAYERS), default=None,
                        help="for a capture that does not state its layer")
    args = parser.parse_args(argv)

    if args.describe:
        with open(args.path, encoding="utf-8") as handle:
            print(json.dumps(capture_file.describe(handle), indent=2))
        return 0

    shown = 0
    for seq, direction, header, size, payload in capture_file.messages(args.path, args.assume_layer):
        if args.header is not None and header != args.header:
            continue
        print(f"{seq:>6} {direction:<4} 0x{header:04X} {size:>5}  {payload.hex()}")
        shown += 1
        if args.limit and shown >= args.limit:
            break
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))

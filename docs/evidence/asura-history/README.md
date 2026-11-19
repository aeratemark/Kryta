# ASURA historical documentation recovery

This is a historical source archive recovered from the local ASURA Git history on 2026-11-14.
It preserves the last committed version found for every Markdown or text path under ASURA `docs/`
across locally available branches. Paths and commit IDs below identify the original. The files
retain their original historical claims, plans, links and status; those must be checked against
Kryta and its current authorities before being acted on. References in the archived files may point
at the original ASURA tree or older documents and are preserved as written.

The consolidation commit is `f7c2f31` (forty-one files become thirteen). Earlier files and
later replacements are both retained. History can contain earlier versions of each listed path;
read them with `git -C <ASURA> log --all -- <path>` and `git -C <ASURA> show <commit>:<path>`.
This index does not claim every historical version has been reviewed or that every historical
assertion remains correct. The current ASURA checkout was `ca9f532` when the archive was created.

One file, `docs/adr/0008-database-vendor-mysql.md`, contained non-ASCII characters. These are
preserved as Python-style Unicode escapes to meet Kryta's ASCII rule. The original Git blob is
identified below and remains available in ASURA. All other archived contents are byte-for-byte
copies of the listed Git blobs.

## Starting points

| Subject | Recovered source | Kryta context |
|---|---|---|
| Game rules | `docs/36-game-rules-ledger.md` | Already recovered as `docs/game-rules.md`. |
| Rust mechanics, Lua behavior, events and scripting | `docs/30-world-content-and-scripting.md`; `docs/guidance/scripting-and-behavior.md` | `docs/port/11-content-and-behaviour.md` owns the recovered design. |
| HeroAI and scaling | `docs/31-heroai-server-feasibility.md`; `docs/research/heroai-server-port.md` | `docs/port/06-world-plan.md` names the hero subject. |
| Self-contained profiles and separate saved state | `docs/34-self-contained-profiles.md`; `docs/guidance/game-packs.md` | `docs/port/11-content-and-behaviour.md` owns the recovered design. |
| Skills, mechanics and behavior | `docs/35-skill-mechanics-and-behavior.md`; `docs/research/skill-mechanics-and-behavior.md` | Game facts and open issues require comparison with `docs/game-rules.md`. |
| World simulation and the control panel | `docs/32-world-simulation-plan.md`; `docs/33-control-panel-design.md`; `docs/work/world-simulation-plan.md`; `docs/work/control-panel-plan.md` | World subjects are mapped in `docs/port/06-world-plan.md`; panel requirements are in `docs/port/12-research-and-control-panel.md`. |
| Captures, initialization and parity | `docs/23-initialization-protocol.md`; `docs/24-stream-parity.md`; `docs/29-initialization-capture.md`; `docs/history/gwps-parity/`; `docs/reference/client-protocol.md` | `docs/evidence/`, `docs/port/02-wire.md`, and `docs/port/09-parity-audit.md` hold Kryta's present evidence and ledger. |
| ASURA reengineering | `docs/work/re-engineering-plan.md`; `docs/research/azerothcore-structure.md`; `docs/37-rework-layers.md` | Compare with Kryta's `docs/port/01-structure.md` and decisions. |

The ongoing cross-document audit and earlier-revision review are tracked in
`docs/port/13-documentation-audit.md`.

## Findings from the first comparison

- `docs/game-rules.md` began with the recovered ledger. It incorporates later ASURA corrections to R11/R12, R22, R27, E199 and E220, and the unresolved world-model questions.
- `docs/evidence/packet-coverage.md` matches the recovered `docs/work/packet-coverage.md` byte for
  byte. `docs/evidence/client-protocol.md` contains a correction to the `0x8038` login message size,
  so the historical copy must not overwrite it.
- `docs/30-world-content-and-scripting.md` records the owner's selection of Lua for external game
  behavior and Rust for simulation mechanics across the whole simulation. It also records
  BehaviorTree.CPP as the execution reference. The integration is not implemented by that document.
- `docs/34-self-contained-profiles.md` records one complete selected profile without fallback to
  other profiles, with separate account settings, characters, levels and progress. It leaves the
  database separation mechanism and packaging details open.
- `docs/31-heroai-server-feasibility.md` contains source analysis and scaling risks for server-side
  HeroAI. `docs/35-skill-mechanics-and-behavior.md` contains skill research and open questions.
  Their findings need a subject-by-subject comparison with Kryta before becoming implementation work.
- The archive preserves 121 paths, including earlier parity, capture, operations, planning and
  decision files. A complete semantic comparison of every historical claim with Kryta is still open.

## Source inventory

Each link opens the recovered file. The commit is the last commit found that added or modified that
path in the available history. The blob identifies the original bytes.

| ASURA path | Last change commit | Git blob |
|---|---|---|
| [docs/00-glossary.md](docs/00-glossary.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `0c978e57f0b205b9c4225294f291a68317395b3d` |
| [docs/01-scope.md](docs/01-scope.md) | `46d0c82fbb7749227997b4978f96b91024f127ef` | `52cc3b5197360ed1c3111278c9432eb6ddae384f` |
| [docs/02-architecture.md](docs/02-architecture.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `1a3160f7b4be5a830a43288ffbb0d1b8b275c091` |
| [docs/03-protocol.md](docs/03-protocol.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `4892cb0e1b88103231bd90bbb8d4ccc6fc99b553` |
| [docs/04-from-gwps.md](docs/04-from-gwps.md) | `44b496550b9a6fec495a478537b05a52d038c11d` | `5cdf64db2b12e79a56ac8a7fdfc62e712b5cec7d` |
| [docs/05-gw2-lessons.md](docs/05-gw2-lessons.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `0aead9dd9c7925da78c5b774d28ef42fc9d3a506` |
| [docs/06-reference-session.md](docs/06-reference-session.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `de4b6b06e50279dd38b2be8c6fe0398302a05180` |
| [docs/07-status.md](docs/07-status.md) | `46d0c82fbb7749227997b4978f96b91024f127ef` | `77953d001037e696ff02e32f60b154a557a471b2` |
| [docs/08-next-inventory-model.md](docs/08-next-inventory-model.md) | `9c6cfdc8d1113144d79563a8e0e1f16ef8612b4d` | `153ad12a04ee09b292479df2c2d326fc4f45ed86` |
| [docs/09-parity-audit.md](docs/09-parity-audit.md) | `9c6cfdc8d1113144d79563a8e0e1f16ef8612b4d` | `8ade80893525a05fc6673e6b7148e3869cf3fb96` |
| [docs/10-parity-queue.md](docs/10-parity-queue.md) | `9c6cfdc8d1113144d79563a8e0e1f16ef8612b4d` | `6d91049a37f666b872c28ddfc102466da162c1d3` |
| [docs/11-parity-state.md](docs/11-parity-state.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `8ca9d53a361da123d2da33736ba7964b1b745e3d` |
| [docs/12-schema-parity.md](docs/12-schema-parity.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `2903d5ef3c9f484916b115e7aa3c1ca38187bd9e` |
| [docs/13-command-parity.md](docs/13-command-parity.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `c610cb618b6a14603ff98b40253c69d652cfae19` |
| [docs/14-test-plan.md](docs/14-test-plan.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `7afd6a5b23e5e806b557abfbae95bb408b9ec412` |
| [docs/15-service-startup.md](docs/15-service-startup.md) | `46d0c82fbb7749227997b4978f96b91024f127ef` | `c1372b32569f6574440dc7c798f33d811ab16999` |
| [docs/16-implementation-plan.md](docs/16-implementation-plan.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `535017c8878e5ef5eb0d8d3811d5846a5f25adc8` |
| [docs/17-running-on-linux.md](docs/17-running-on-linux.md) | `55f9b5af5fbaf1546cab2fb51c70773817a0a0ef` | `f07cde9790692ad9580c2b58a72a7982fadaa007` |
| [docs/18-multi-machine.md](docs/18-multi-machine.md) | `6b4b192daded893a65acb4b7855220a6d039b581` | `0098faa1412a18250997ac563016251a0e66cb2b` |
| [docs/19-running.md](docs/19-running.md) | `1f4787525bc1b5f7a33c67c0371b4beeff92d398` | `be65b4b271f1ebc61be6199a25b5f12fc62eee9d` |
| [docs/20-rurik-gap.md](docs/20-rurik-gap.md) | `416b22830968678e81683a681a22252b88449aa9` | `f158589d2d7fd22b0793230cf42d9eb4ad55b41c` |
| [docs/21-surrounding-packets.md](docs/21-surrounding-packets.md) | `9c6cfdc8d1113144d79563a8e0e1f16ef8612b4d` | `bd65c41ebdd40023649e70bf635c96f205872647` |
| [docs/22-world-model.md](docs/22-world-model.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `fe518a155e8579ac4cf1ae4abf890487ce391ec0` |
| [docs/23-initialization-protocol.md](docs/23-initialization-protocol.md) | `9c6cfdc8d1113144d79563a8e0e1f16ef8612b4d` | `8af291de590b9a654bcd00f6a01caba46a6a1dd4` |
| [docs/24-stream-parity.md](docs/24-stream-parity.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `7580d9b7094a5fb021c8e6f1395abd429757f282` |
| [docs/25-client-readiness.md](docs/25-client-readiness.md) | `9818bf2d7813dcf4bbc3e17f208e9688c11917ec` | `acad313aeeb2abbf0c0fbd19b5a18cbee9b9b68f` |
| [docs/26-packet-gaps.md](docs/26-packet-gaps.md) | `9818bf2d7813dcf4bbc3e17f208e9688c11917ec` | `0f818d1ee193333a418846ffc3554dc8acd51731` |
| [docs/27-packet-parity-attempt.md](docs/27-packet-parity-attempt.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `8130773a1dc17d232aad8cf07e0390ae296e9803` |
| [docs/28-headers-to-name.md](docs/28-headers-to-name.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `27ae6ce918342defd335f91063b0982f9122f374` |
| [docs/29-initialization-capture.md](docs/29-initialization-capture.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `d87c1df863988786d50a26a7a36702e0376966b4` |
| [docs/30-world-content-and-scripting.md](docs/30-world-content-and-scripting.md) | `416b22830968678e81683a681a22252b88449aa9` | `2b5c865c4a419cb56b876df74fd2e926dd402df9` |
| [docs/31-heroai-server-feasibility.md](docs/31-heroai-server-feasibility.md) | `416b22830968678e81683a681a22252b88449aa9` | `b371022318839085ac88d3a7fdf0eeebaa461c88` |
| [docs/32-world-simulation-plan.md](docs/32-world-simulation-plan.md) | `416b22830968678e81683a681a22252b88449aa9` | `245112b619e59baedb0ba5d31e9a1a793ec21e1f` |
| [docs/33-control-panel-design.md](docs/33-control-panel-design.md) | `bc58da115d0698d3f13db69ee13e6d0ca8654dbc` | `c68f635c55e38402b7c9daeba7e84f64c464a026` |
| [docs/34-self-contained-profiles.md](docs/34-self-contained-profiles.md) | `416b22830968678e81683a681a22252b88449aa9` | `7b3297033ea8d75c17605b93523ad0d29e7de680` |
| [docs/35-skill-mechanics-and-behavior.md](docs/35-skill-mechanics-and-behavior.md) | `416b22830968678e81683a681a22252b88449aa9` | `ce02e5837be1f385aad02ad4b8e5380054dee6e2` |
| [docs/36-game-rules-ledger.md](docs/36-game-rules-ledger.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `2c7270ce5e851ca3b2fd8c3da9ec079dc394580a` |
| [docs/37-rework-layers.md](docs/37-rework-layers.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `5720857f6a797b4e9eeca11d23c28266dca7fcbb` |
| [docs/38-layer-decisions.md](docs/38-layer-decisions.md) | `90feb5a7a7769080e2a8529709f99759c4e7a1a3` | `cdc061e919e949c33ab073ff9e08d43f1566b4d7` |
| [docs/39-map-list.md](docs/39-map-list.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `d2693173ba8756d7de8ab6f72aec520e669e5779` |
| [docs/40-guidelines.md](docs/40-guidelines.md) | `1f4787525bc1b5f7a33c67c0371b4beeff92d398` | `805078824e7897f2ad76167a7ab35e65a9d7fb62` |
| [docs/HANDOFF.md](docs/HANDOFF.md) | `1f4787525bc1b5f7a33c67c0371b4beeff92d398` | `85d40fb427103907cfcb08e291e7bd3e0d1fe3e9` |
| [docs/README.md](docs/README.md) | `bb7a7efbe7aa2ba2fc6e1fee3aede9ac2a21c633` | `293b83f3991dea5a9774231f5dbb6adf05422777` |
| [docs/adr/0001-rust.md](docs/adr/0001-rust.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `ba4b57431f34a960ca7613745dee8f6b1b9d6659` |
| [docs/adr/0002-separate-project.md](docs/adr/0002-separate-project.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `10bf6fa568c1e3be7233e19ddf520cfc4db8a5c9` |
| [docs/adr/0003-simulation-in-scope.md](docs/adr/0003-simulation-in-scope.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `a95d1f5e510802467ac153ee2b8aeaa9590b81e6` |
| [docs/adr/0004-port-azerothcore-structure.md](docs/adr/0004-port-azerothcore-structure.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `896c81eff6690971161183d946e3b7dffaafd653` |
| [docs/adr/0005-gwps-reference-only.md](docs/adr/0005-gwps-reference-only.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `81e74dd7a0832c7344c169816bb31594542a8f68` |
| [docs/adr/0006-azerothcore-is-the-reference.md](docs/adr/0006-azerothcore-is-the-reference.md) | `68e2fd290d951b331c28db48954a2972ed373599` | `44db73a32a8acec6bf47c13195fc3516c6384524` |
| [docs/adr/0007-rust-translation.md](docs/adr/0007-rust-translation.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `99953ae7494640fdb659ef279da18a52725ec4f8` |
| [docs/adr/0008-database-vendor-mysql.md](docs/adr/0008-database-vendor-mysql.md) | `a8403f992302298480d14805383c506ee6a1cb5b` | `45a77b6ec56798cd303fca266c0d75ba295d672a` (escaped) |
| [docs/adr/0008-database-vendor.md](docs/adr/0008-database-vendor.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `84a3479f9d13dc7c01d754b0c19117561bb14b33` |
| [docs/adr/0009-crypto-primitives.md](docs/adr/0009-crypto-primitives.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `a9c5c3d69f1c68d4617e82717fb4e5a685376869` |
| [docs/adr/0010-gwps-is-the-floor.md](docs/adr/0010-gwps-is-the-floor.md) | `e571053bd1ef6f8611771992958e717e6125d014` | `d200331e1589919e09970162dca5ab222f86b639` |
| [docs/adr/0011-rurik-is-a-source.md](docs/adr/0011-rurik-is-a-source.md) | `1fcf0a535aa76116b1935522937dbc3419208d3e` | `9db74334f0b2784f2b6001b9201ae9bdf683214f` |
| [docs/adr/0012-eden-is-a-source.md](docs/adr/0012-eden-is-a-source.md) | `23bb4071ec02425f064f73335bd3ede865056bd3` | `392c95eb42fdbbb0b00c1de7f94dfcbaa96877d2` |
| [docs/adr/0013-py4gw-stealth-is-our-link-to-the-client.md](docs/adr/0013-py4gw-stealth-is-our-link-to-the-client.md) | `23bb4071ec02425f064f73335bd3ede865056bd3` | `b650f99c26bd8bc4466fbae2df2c0be51596204c` |
| [docs/adr/0014-behavior-tree-execution.md](docs/adr/0014-behavior-tree-execution.md) | `416b22830968678e81683a681a22252b88449aa9` | `96e6e37327754eb1fe521b6d15ef41e6d57c683b` |
| [docs/guidance/architecture.md](docs/guidance/architecture.md) | `e2fc5db51b46408c7397f647182fa9f557ff3856` | `ea221e1635fe4f0f77366ab3e45456313825ed51` |
| [docs/guidance/game-packs.md](docs/guidance/game-packs.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `5ee7031521c4c620b8e1934be7ef106b7425c02d` |
| [docs/guidance/scripting-and-behavior.md](docs/guidance/scripting-and-behavior.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `851964a1587b492cb62aed6d16670852c36513c1` |
| [docs/history.md](docs/history.md) | `1f4787525bc1b5f7a33c67c0371b4beeff92d398` | `879bb8e004e1406b5b6b4093a8c8d22b1d3762f8` |
| [docs/history/development-incidents.md](docs/history/development-incidents.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `6fe932a9a091bd02dc2a76a636116fcf0b95ac54` |
| [docs/history/gwps-parity/behavior-audit.md](docs/history/gwps-parity/behavior-audit.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `009e4c4d97fd9dd691042afa8fdc88166ca49529` |
| [docs/history/gwps-parity/chat-command-comparison.md](docs/history/gwps-parity/chat-command-comparison.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `31e3c5d8499e3f1df4d7368a59db3ec3235e5da5` |
| [docs/history/gwps-parity/completed-work-queue.md](docs/history/gwps-parity/completed-work-queue.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `1f9305e4ac6d534ce7374aada141b1f375666501` |
| [docs/history/gwps-parity/database-schema-comparison.md](docs/history/gwps-parity/database-schema-comparison.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `5b7440f67de023687f4b8b086ee2bf9836419b24` |
| [docs/history/gwps-parity/handler-status.md](docs/history/gwps-parity/handler-status.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `0540451848acfea43e9ad6a3588f38cc7d3723fd` |
| [docs/history/gwps-parity/parity-snapshot-2026-10-31.md](docs/history/gwps-parity/parity-snapshot-2026-10-31.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `2b9c1f0e5c0afb641c7958586fcabeaae1ce659b` |
| [docs/history/gwps-parity/reference-test-coverage.md](docs/history/gwps-parity/reference-test-coverage.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `2bd10136e00649320b704734f0b1821d239fe79b` |
| [docs/history/gwps-project-origins.md](docs/history/gwps-project-origins.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `2b096135628d08dfc9b4870fc46da92cb479c863` |
| [docs/history/infrastructure-milestones.md](docs/history/infrastructure-milestones.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `4ff44405e4e36ff2eecae1072d3305b6560b4b24` |
| [docs/history/packet-parity-attempt-2026-10-31.md](docs/history/packet-parity-attempt-2026-10-31.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `94477608cda20697875d06cc0763f4f1ef7bd835` |
| [docs/history/reviews/documentation-organization-2026-11-07.md](docs/history/reviews/documentation-organization-2026-11-07.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `ef4f28b06fc3910aabcdb6b537d9c5d47eb9d3d4` |
| [docs/history/service-supervisor-delivery-plan.md](docs/history/service-supervisor-delivery-plan.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `e8ab60a283e1b41eaa6a7392752c7688fcee54d1` |
| [docs/history/service-supervisor-research.md](docs/history/service-supervisor-research.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `61233eee0603b03a72e5bce1da3afda8b034148f` |
| [docs/operations/capturing-instance-initialization.md](docs/operations/capturing-instance-initialization.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `b299aff118dc5a46a082ce1947bb68914c872e54` |
| [docs/operations/running-on-linux.md](docs/operations/running-on-linux.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `82eccff771d3fbaf32bce4e7b1e8716d44f24e93` |
| [docs/operations/running-the-server.md](docs/operations/running-the-server.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `d711eb7b0fec4cf4cab984ff7b5c7d406c2715f4` |
| [docs/plans.md](docs/plans.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `8859598450f432a6d1e29b1e25e524077f2f963c` |
| [docs/reference/captures/login-session-2026-10-26.md](docs/reference/captures/login-session-2026-10-26.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `26f14a844a43487e9b01059727c39391aa0ffafb` |
| [docs/reference/client-protocol.md](docs/reference/client-protocol.md) | `e2fc5db51b46408c7397f647182fa9f557ff3856` | `2db497565b0aade3f8a8a03f9bc882108b256702` |
| [docs/reference/decisions/0001-control-plane-language.md](docs/reference/decisions/0001-control-plane-language.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `31361f3d734b8669dbf4cf52292908d111884477` |
| [docs/reference/decisions/0002-project-separation.md](docs/reference/decisions/0002-project-separation.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `7711208b0921f718d32b43bd4b2ddfee99b3f9df` |
| [docs/reference/decisions/0003-simulation-scope.md](docs/reference/decisions/0003-simulation-scope.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `2ec35a7577d74e69f43816057ece693f34111062` |
| [docs/reference/decisions/0004-server-structure.md](docs/reference/decisions/0004-server-structure.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `b9a4ef0b14d851df6bc37dd12f1ae48fe82b9ae2` |
| [docs/reference/decisions/0005-gwps-reference-boundary.md](docs/reference/decisions/0005-gwps-reference-boundary.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `a2c034a51f093f9964624cebd2de4f2969a77195` |
| [docs/reference/decisions/0006-structural-design-authority.md](docs/reference/decisions/0006-structural-design-authority.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `1e3d377940eae55138907312b897338ace13c2c8` |
| [docs/reference/decisions/0007-whole-stack-rust.md](docs/reference/decisions/0007-whole-stack-rust.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `6d86ebcff8930213dca37b118711bf3cb6f37885` |
| [docs/reference/decisions/0008-postgresql.md](docs/reference/decisions/0008-postgresql.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `063fd89afddfe659741d86f645bb59f57c7de3f0` |
| [docs/reference/decisions/0009-protocol-cryptography.md](docs/reference/decisions/0009-protocol-cryptography.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `7d9ed2c02cc7405dd5b908bfb88de51b39033930` |
| [docs/reference/decisions/0010-gwps-behavior-coverage.md](docs/reference/decisions/0010-gwps-behavior-coverage.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `210b47efefe578d89372c634512371a139984119` |
| [docs/reference/decisions/0011-rurik-source-policy.md](docs/reference/decisions/0011-rurik-source-policy.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `9db74334f0b2784f2b6001b9201ae9bdf683214f` |
| [docs/reference/decisions/0012-eden-source-policy.md](docs/reference/decisions/0012-eden-source-policy.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `5c0a53458135948734014ed5955b53622d3128ac` |
| [docs/reference/decisions/0013-py4gw-client-research-policy.md](docs/reference/decisions/0013-py4gw-client-research-policy.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `944d03612e156a9bab754560f0631c13aad103a3` |
| [docs/reference/decisions/0014-behavior-tree-execution.md](docs/reference/decisions/0014-behavior-tree-execution.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `cf7518e0160e4673025582ed36059634a4e86693` |
| [docs/reference/decisions/layer-decisions.md](docs/reference/decisions/layer-decisions.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `29613c8c886207cea2191272001e29de2a5bac82` |
| [docs/reference/decisions/project-decisions.md](docs/reference/decisions/project-decisions.md) | `01250ff46bbe8a3a0953345256b35de8f98e6989` | `b7766c8c45ea1927f35d8a78a3e5c89abe0c230d` |
| [docs/reference/game-world-model.md](docs/reference/game-world-model.md) | `01250ff46bbe8a3a0953345256b35de8f98e6989` | `ba6e96f1dcb6721de359a193471ca048de135683` |
| [docs/reference/glossary.md](docs/reference/glossary.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `91716c489246b283007b07aa500ba963d2d81db8` |
| [docs/reference/instance-initialization.md](docs/reference/instance-initialization.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `f1e239fdc70139f8221b8fb00d55ef49ad37c982` |
| [docs/reference/inventory-model.md](docs/reference/inventory-model.md) | `e2fc5db51b46408c7397f647182fa9f557ff3856` | `67baf2f818112b69a46df1b395926ba6a46e6efa` |
| [docs/reference/map-catalogue.md](docs/reference/map-catalogue.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `c11b35a66fae29f71c579935f98eb4660754f0ba` |
| [docs/research/azerothcore-structure.md](docs/research/azerothcore-structure.md) | `bb7a7efbe7aa2ba2fc6e1fee3aede9ac2a21c633` | `5c9ba3728c0df4773cc0b8f40b513266eaf1bc91` |
| [docs/research/client-session-packet-analysis.md](docs/research/client-session-packet-analysis.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `291e0c6a201a1e117c330bcb13326a0cdfd7a492` |
| [docs/research/gw2-service-design-lessons.md](docs/research/gw2-service-design-lessons.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `2b12a65fab514bdc035d039282ddaae1c2f28fcd` |
| [docs/research/heroai-server-port.md](docs/research/heroai-server-port.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `ccaa029b0bcea9c248384cb022d7f0c86f696e26` |
| [docs/research/multi-host-routing-experiment.md](docs/research/multi-host-routing-experiment.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `c32f8265f20e849b36fc89b2e24ed3f2dce85dad` |
| [docs/research/rurik-capabilities-and-gaps.md](docs/research/rurik-capabilities-and-gaps.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `34a1eb84561b1ee684d5eb4c26e6c45053ea0eb5` |
| [docs/research/skill-mechanics-and-behavior.md](docs/research/skill-mechanics-and-behavior.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `cc61ebfb77a53f37d1417ea0ea73f40a4c202e2c` |
| [docs/roadmap.md](docs/roadmap.md) | `f7c2f313b32bbf0256496edb316716c0f279be7d` | `fe93f7229c9f4ff16cc53621159e79647784883a` |
| [docs/work/control-panel-plan.md](docs/work/control-panel-plan.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `de822a7abffd8645d904bdaaa80f2390ac2444a3` |
| [docs/work/documentation-consistency.md](docs/work/documentation-consistency.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `4e1a9dbcfa35afea26afab5b9e813635774422c7` |
| [docs/work/layer-rework-checklist.md](docs/work/layer-rework-checklist.md) | `e2fc5db51b46408c7397f647182fa9f557ff3856` | `efebed92d01198133e102ab8714d7265a6a73021` |
| [docs/work/packet-coverage.md](docs/work/packet-coverage.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `07ae002272c4727015ebf6e5fd346cbbddb145ce` |
| [docs/work/project-roadmap.md](docs/work/project-roadmap.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `27611f09bce4a104db20c330d7c9670b9984d483` |
| [docs/work/project-status.md](docs/work/project-status.md) | `e2fc5db51b46408c7397f647182fa9f557ff3856` | `3485e3057090d2a8bb669d69a16b99f929f5b6d9` |
| [docs/work/re-engineering-plan.md](docs/work/re-engineering-plan.md) | `ca9f532a54f55f51e80076b656f489fc9f734f4c` | `e0ac62c17c0e7d2f4e2830f31bec3c780e03ee84` |
| [docs/work/session-handoff.md](docs/work/session-handoff.md) | `d7d229d074ef624405f67adbee0db5da50fd0a60` | `8d538421b20fd4b9241804eecb6f3eb09e1626cd` |
| [docs/work/unnamed-packets.md](docs/work/unnamed-packets.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `08a18e5699852eb91b7e42a2346fe096339766ec` |
| [docs/work/world-simulation-plan.md](docs/work/world-simulation-plan.md) | `f7d0840a17c257bbed4010a5a577ef1c6d4a1efe` | `2207afcc3bdc0a3b43004de693c8fed9a1fdb0a1` |

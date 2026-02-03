# verify-contract

Verify deployed smart contract bytecode against source code by compiling a Solidity standard JSON input and comparing the result to on-chain bytecode.

The comparison automatically accounts for:
- Library address placeholders
- Immutable variable values
- Library self-address guards
- CBOR-encoded compiler metadata

## Usage

```
verify-contract [OPTIONS] --standard-json-input <PATH> --contract-name <NAME> <ADDRESS_OR_BYTECODE>
```

### Positional argument

`<ADDRESS_OR_BYTECODE>` is auto-detected:

| Input | Behavior |
|---|---|
| `0x` + 40 hex chars | Treated as an Ethereum address; bytecode is fetched via `eth_getCode` |
| Existing file path | Read as a hex-encoded bytecode file |
| Anything else | Treated as inline hex bytecode |

### Options

| Flag | Short | Description |
|---|---|---|
| `--standard-json-input <PATH>` | `-i` | Path to solc standard JSON input file (required) |
| `--contract-name <NAME>` | `-c` | Contract name to verify (required) |
| `--solc <VERSION_OR_PATH>` | `-s` | Solc version (e.g. `0.8.27`) or path (e.g. `/usr/bin/solc`). Omit to use `solc` from PATH |
| `--rpc-url <URL>` | `-r` | RPC endpoint URL (default: `http://localhost:8545`, env: `RPC_URL`) |
| `-v` | | Verbosity (`-v` error, `-vv` warn, `-vvv` info, `-vvvv` debug, `-vvvvv` trace) |

## Examples

Verify against a bytecode file:

```sh
verify-contract \
  -i input.json \
  -c MyContract \
  -s 0.8.27 \
  path/to/onchain.bytecode
```

Verify against a deployed contract address:

```sh
verify-contract \
  -i input.json \
  -c MyContract \
  -s 0.8.27 \
  -r https://mainnet.megaeth.com/rpc \
  0xabc...def
```

Verify with inline hex bytecode:

```sh
verify-contract \
  -i input.json \
  -c MyContract \
  0x6080604052...
```

## Build

```sh
cargo build --release
```

## Test

```sh
cargo test
```

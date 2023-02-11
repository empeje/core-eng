# btctool

## How to consolidate UTXOs

The UTXO consolidation tool:
  - makes API queries to the BitGo and Trezor APIs. 
  - does not broadcast any transactions. 
  - does not store any private keys. 
  - does not store any API keys.
  - supports P2WPKH for input and output addresses.

### Instructions
 
- Pull the docker image
    ```bash
    docker pull igorsyl/btctool:latest
    ```
- Generate the consolidation transaction. Type this command into a bash file, edit as necessary, and execute:
    ```bash
    docker run -it -v /tmp:/tmp igorsyl/btctool:latest \
      consolidate \
      --input-address=$BTC_ADDRESS \
      --output-address=$BTC_ADDRESS \
      --utxo-fetch-limit=10000 \
      --utxo-max-count=10000 \
      --utxo-max-value=100000 \
      --spend-hd-path="m/49'/0'/0'/0/0" \
      --est-fee-sats-per-vbyte=10 \
      --trezor-tx-file=/tmp/trezor_tx.json
    ```
- The Trezor JSON unsigned transaction is written to `trezor_tx.json`:
  ```bash
  less /tmp/trezor_tx.json
  ``` 
- Sign the transaction using the Trezor CLI.
  - Instructions to install the Trezor CLI: https://trezor.io/learn/a/trezorctl-on-macos
  - Sign the transaction:
  ```bash
  trezorctl btc sign-tx trezor_tx.json
  ```
- The signed raw transaction is displayed to the console.
- Verify the signed transaction using a tool like https://coinb.in/#verify
- Broadcast the signed transaction using a tool like https://coinb.in/#broadcast

Optionally, remove the `--sign` flag to generate the unsigned transaction only.
- Use the Trezor Suite to sign the transaction.
- Or use the Trezor Python library to sign the transaction:
```bash
  docker run -it -v tx:/app/tx igorsyl/btctool:latest \
    sign --trezor-tx-file=tx/trezor_tx.json 
```

## Development

### Example addresses
- The burn address: 1111111111111111111114oLvT2
- A random address: 14CEjTd5ci3228J45GdnGeUKLSSeCWUQxK

### Setting up a Local Dev Environment

Clone the repo and install the dependencies
```bash
gh clone Trust-Machines/btctool; cd btctool
sh install.sh
```

### Running the consolidation tool
```bash
./btctool-cli consolidate \
      --input-address=$BTC_ADDRESS \
      --output-address=$BTC_ADDRESS \
      --utxo-fetch-limit=10000 \
      --utxo-max-count=10000 \
      --utxo-max-value=100000 \
      --spend-hd-path="m/49'/0'/0'/0/0" \
      --est-fee-sats-per-vbyte=10 \
      --trezor-tx-file=trezor_tx.json
```

Use the following flags to enable caching for BitGo and Trezor API calls:
```commandline
--utxo-use-cache
--utxo-data-use-cache
```


### Building and Publishing the Docker image

- Login to Docker
  ```bash 
  docker login
  ```
- Build and pish the docker image
  ```bash 
  docker build -t igorsyl/btctool:latest . && docker push igorsyl/btctool:latest
  ```

### Trezor Firmware

- Checkout the trezor-firmware repository patched to sign arbitrary transaction inputs
  - `gh repo clone Trust-Machines/trezor-firmware` 
  - `gh pr checkout https://github.com/Trust-Machines/trezor-firmware/pull/1`
- Build the trezor emulator - https://docs.trezor.io/trezor-firmware/core/build/emulator.html 
  - Mac: `brew install scons sdl2 sdl2_image pkg-config llvm`
  - `rustup install nightly && rustup default nightly && rustup update`
  - `poetry shell`
  - `poetry install`
  - `cd core; make vendor build_unix`
- Run the emulator: `./emu.sh -e core`

### References
- https://api.bitgo.com/docs/#tag/Overview
- https://github.com/trezor/trezor-firmware/tree/master/python/tools
- https://docs.trezor.io/trezor-suite/
- https://github.com/trezor/trezor-firmware/tree/master/python/
- https://github.com/trezor/trezor-firmware/blob/master/common/protob/messages-bitcoin.proto
- https://github.com/trezor/trezor-firmware/blob/master/python/docs/transaction-format.md
- curl "https://www.bitgo.com/api/v1/address/1111111111111111111114oLvT2/unspents?limit=1&skip=158272" | jq # found 155_000
- curl "https://www.bitgo.com/api/v1/address/14CEjTd5ci3228J45GdnGeUKLSSeCWUQxK/unspents?limit=5000&skip=0" | jq
- curl "https://www.bitgo.com/api/v1/tx/e20185c66e904c3589f341e0303208d8806ad4bcbb0b6b79c62562626fdfa39c" | jq
- curl "https://www.bitgo.com/api/v1/tx/e20185c66e904c3589f341e0303208d8806ad4bcbb0b6b79c62562626fdfa39c" | jq
- curl -A trezorlib "https://btc1.trezor.io/api/tx-specific/e20185c66e904c3589f341e0303208d8806ad4bcbb0b6b79c62562626fdfa39c" | jq

import json

import click
from trezorlib import btc, protobuf
from trezorlib import messages
from trezorlib.client import get_default_client


def sign(trezor_tx):
    client = get_default_client()
    data = trezor_tx
    coin = data["coin_name"]
    details = data.get("details", {})
    inputs = [
        protobuf.dict_to_proto(messages.TxInputType, i) for i in data.get("inputs", ())
    ]
    outputs = [
        protobuf.dict_to_proto(messages.TxOutputType, output)
        for output in data.get("outputs", ())
    ]
    prev_txes = {
        bytes.fromhex(txid): protobuf.dict_to_proto(messages.TransactionType, tx)
        for txid, tx in data.get("prev_txes", {}).items()
    }

    _, serialized_tx = btc.sign_tx(
        client,
        coin,
        inputs,
        outputs,
        prev_txes=prev_txes,
        **details,
    )

    return serialized_tx.hex()


@click.command('sign')
@click.option('--trezor-tx-file', type=str, default='trezor_tx.json')
def cmd_sign(trezor_tx_file):
    click.echo(f'Signing transaction {trezor_tx_file=}')
    with open(trezor_tx_file, 'r') as f:
        trezor_tx = json.load(f)

    serialized_tx = sign(trezor_tx)
    click.echo("Signed Transaction:")
    click.echo(serialized_tx.hex())

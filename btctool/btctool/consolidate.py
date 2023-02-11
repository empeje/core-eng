import json

import sign_command
import click
from trezorlib.protobuf import to_dict
import tx_utils
import utxo_utils
import pickle


@click.command('consolidate')
@click.option('--input-address', type=str, help='Supported: SPENDP2SHWITNESS (P2WPKH)')
@click.option('--output-address', type=str, default=None, help='Suported: PAYTOP2SHWITNESS (P2WPKH). If not specified, the from_address is used.')

@click.option('--utxo-use-cache', is_flag=True, default=False, help='Use cached UTXOs. Default: False')
@click.option('--utxo-cache-file', type=str, default='utxos.json', help='File to store the cached UTXOs. Default: utxos.json')
@click.option('--utxo-fetch-limit', type=int, default=10, help='Maximum number of UTXOs to fetch. Default: 10')
@click.option('--utxo-max-count', type=int, default=10, help='Maximum number of UTXOs to include in consolidation. Default: 10')
@click.option('--utxo-max-value', type=int, default=100_000, help='Maximum value of UTXOs include in consolidation. Default: 100,000 sats')

@click.option('--utxo-data-use-cache', is_flag=True, default=False, help='Use cached transaction. Default: False')
@click.option('--utxo-data-cache-file', type=str, default='utxo-data-cache.bin', help='File to store the cached transaction. Default: txin.json')

@click.option('--spend-hd-path', type=str, default="m/49'/0'/0'/0/0", help='HD path to use for signing the transaction. Default: m/49\'/0\'/0\'/0/0 (SPENDP2SHWITNESS)')
@click.option('--est-fee-sats-per-vbyte', type=int, default=10, help='Estimated fee per vbyte. Default: 10 sats/vbyte. (This is a very rough estimate.)')
@click.option('--trezor-tx-file', type=str, default='trezor_tx.json', help='File to store the Trezor transaction. Default: trezor_tx.json')
@click.option('--sign', is_flag=True, default=False, help='Sign the transaction with the Trezor. Default: False')
def cmd_consolidate(
        input_address, output_address,
        utxo_use_cache, utxo_cache_file, utxo_fetch_limit, utxo_max_count, utxo_max_value,
        utxo_data_use_cache, utxo_data_cache_file,
        spend_hd_path, est_fee_sats_per_vbyte, trezor_tx_file, sign):
    output_address = output_address or input_address
    click.echo(f'Consolidate UTXOs {input_address=} {output_address=} {spend_hd_path=} {est_fee_sats_per_vbyte=}')

    if not input_address:
        raise click.ClickException('Please specify an input address')

    if utxo_use_cache:
        click.echo(f'Using cached UTXOs from {utxo_cache_file=}')
        with open(utxo_cache_file, 'r') as f:
            utxos = json.load(f)
    else:
        # click.echo(f'Fetching UTXOs from the Bitgo API {utxo_fetch_limit=}')
        utxos = utxo_utils.fetch_utxos(input_address, utxo_fetch_limit)
        with open(utxo_cache_file, 'w') as f:
            json.dump(utxos, f, indent=2)
    click.echo(f'Found {len(utxos):,} UTXOs')

    utxos.sort(key=lambda u: u['value'], reverse=True)
    utxos = utxos[:utxo_max_count]
    utxos = filter(lambda u: u['value'] > utxo_max_value, utxos)
    utxos = list(utxos)
    utxos_amount = sum([utxo['value'] for utxo in utxos])
    click.echo(f'Filtered {len(utxos):,} eligible UTXOs for consolidation amounting to {utxos_amount:,} sats {utxo_max_count=:,} {utxo_max_value=:,}')

    if utxo_data_use_cache:
        click.echo(f'Using cached transaction inputs from {utxo_data_cache_file=}')
        with open(utxo_data_cache_file, 'rb') as f:
            utxo_data_list = pickle.load(f)
    else:
        # click.echo(f'Fetching input transactions using the Trezor API')
        utxo_data_list = tx_utils.fetch_utxo_transactions(utxos)
        with open(utxo_data_cache_file, 'wb') as f:
            pickle.dump(utxo_data_list, f)

    click.echo('Creating the consolidation transaction inputs')
    for utxo_data in utxo_data_list:
        tx_utils.hydrate_utxo_data(utxo_data, spend_hd_path)
    utxo_input_tx_amount = sum([utxo_data['input'].amount for utxo_data in utxo_data_list])
    assert (utxos_amount == utxo_input_tx_amount, f'{utxos_amount=} {utxo_input_tx_amount=}')
    tx_inputs = [utxo_data['input'] for utxo_data in utxo_data_list]

    click.echo('Creating the consolidation transaction outputs')
    estimated_fee = est_fee_sats_per_vbyte * 100 * len(utxo_data_list)
    tx_output_amount = utxo_input_tx_amount - estimated_fee
    tx_outputs = tx_utils.get_outputs(output_address, tx_output_amount)
    click.echo(f'Created {len(tx_outputs)} outputs')

    coin = 'Bitcoin'
    version = 2
    lock_time = 0

    trezor_tx = {
        "coin_name": coin,
        "inputs": [to_dict(i, hexlify_bytes=True) for i in tx_inputs],
        "outputs": [to_dict(o, hexlify_bytes=True) for o in tx_outputs],
        "details": {
            "version": version,
            "lock_time": lock_time,
        },
        "prev_txes": {
            utxo_data['txhash']: to_dict(utxo_data['tx'], hexlify_bytes=True)
            for utxo_data in utxo_data_list
        },
    }

    click.echo(f'Writing transaction {trezor_tx_file=}')
    with open(trezor_tx_file, 'w') as f:
        json.dump(trezor_tx, f, sort_keys=True, indent=2)

    if sign:
        click.echo(f'Signing transaction {trezor_tx_file=}')
        with open(trezor_tx_file, 'r') as f:
            trezor_tx = json.load(f)

        try:
            serialized_tx = sign_command.sign(trezor_tx)
            click.echo("Signed Transaction:")
            click.echo(serialized_tx.hex())
        except Exception as e:
            click.echo(f'Error: {e}')


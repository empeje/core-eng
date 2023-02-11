import decimal
import multiprocessing
import time

import requests
import tqdm
import trezorlib
import trezorlib.btc

import utils

# the following script type mapping is only valid for single-sig Trezor-generated utxos
BITCOIN_CORE_INPUT_TYPES = {
    "pubkeyhash": trezorlib.messages.InputScriptType.SPENDADDRESS,
    "scripthash": trezorlib.messages.InputScriptType.SPENDP2SHWITNESS,
    "witness_v0_keyhash": trezorlib.messages.InputScriptType.SPENDWITNESS,
    "witness_v1_taproot": trezorlib.messages.InputScriptType.SPENDTAPROOT,
}


def get_session():
    session = requests.Session()
    session.headers.update({"User-Agent": "trezorlib"})
    return session


class RateLimitException(Exception):
    pass


def get_outputs(address, amount):
    outputs = []
    address_n = None
    script_type = trezorlib.messages.OutputScriptType.PAYTOP2SHWITNESS

    outputs.append(
        trezorlib.messages.TxOutputType(
            amount=amount,
            address_n=address_n,
            address=address,
            script_type=script_type,
        )
    )

    return outputs


def hydrate_utxo_data(utxo_data, hd_path):
    utxo = utxo_data['utxo']
    tx_json = utxo_data['tx_json']
    utxo_hash = bytes.fromhex(utxo['txid'])
    txhash = utxo_hash.hex()
    tx = trezorlib.btc.from_json(tx_json)
    amount = utxo['value']
    address_n = trezorlib.tools.parse_path(hd_path)
    utxo_index = int(utxo['output_n'])
    reported_type = tx_json["vout"][utxo_index]["scriptPubKey"]["type"]
    script_type = BITCOIN_CORE_INPUT_TYPES[reported_type]
    sequence = 0xFFFFFFFD

    new_input = trezorlib.messages.TxInputType(
        address_n=address_n,
        prev_hash=utxo_hash,
        prev_index=utxo_index,
        amount=amount,
        script_type=script_type,
        sequence=sequence,
    )

    utxo_data.update({'input': new_input, 'tx': tx, 'txhash': txhash})

def fetch_utxo_transaction(utxo, request_url):
    utxo_txid = bytes.fromhex(utxo['txid'])
    txhash = utxo_txid.hex()
    request_url = f'{request_url}/{txhash}'
    r = get_session().get(request_url, timeout=1)
    if not r.ok:
        # raise Exception(tx_url, r.content)
        # click.echo(f'Got HTTP status code {r.status_code} for {tx_url=}')
        raise RateLimitException(request_url, r.content)

    tx_json = r.json(parse_float=decimal.Decimal)
    return {'utxo': utxo, 'tx_json': tx_json}


@utils.ignore_keyboard_interrupt
def request_worker(queue, result_list, error_list, worker_index):
    request_url = f'https://btc{worker_index}.trezor.io/api/tx-specific'
    # request_url = f'https://www.bitgo.com/api/v1/tx'

    min_timeout_secs = 0.5
    max_timeout_secs = 0.5

    timeout_secs = min_timeout_secs
    while True:
        utxo = queue.get()
        try:
            result_list.append(fetch_utxo_transaction(utxo, request_url))
            timeout_secs = min_timeout_secs
            time.sleep(timeout_secs)
        # except RateLimitException as ex:
        except Exception as ex:
            queue.put(utxo)
            error_list.append(ex)
            timeout_secs = min(max_timeout_secs, 2*timeout_secs)
            time.sleep(timeout_secs)
        finally:
            queue.task_done()


def fetch_utxo_transactions(utxos):
    manager = multiprocessing.Manager()
    queue = manager.Queue()
    result_list = manager.list()
    error_list = manager.list()
    procs = []

    for worker_index in range(1, 6):
        proc = multiprocessing.Process(target=request_worker, args=[queue, result_list, error_list, worker_index])
        procs.append(proc)

    for utxo in utxos:
        queue.put(utxo)

    for proc in procs:
        proc.start()

    result_count = 0
    error_count = 0
    with tqdm.tqdm(total=len(utxos), desc=f'Fetching {len(utxos):,} transactions from the Trezor API') as bar:
        while queue.qsize():
            new_result_count = len(result_list)
            new_error_count = len(error_list)
            if new_result_count > result_count or new_error_count > error_count:
                bar.set_description(f'Fetched {result_count:,} transactions from the Trezor API (errors: {len(error_list):,})')
                bar.update(new_result_count-result_count)
                result_count = new_result_count
                error_count = new_error_count
            time.sleep(0.1)

    bar.close()
    queue.join()

    for proc in procs:
        proc.terminate()

    return list(result_list)


'''
def fetch_inputs_single(utxos):
    inputs = []
    txes = {}
    request_url = f'https://btc1.trezor.io/api/tx-specific'
    while utxos:
        utxo = utxos.pop()
        try:
            response = get_input(utxo, request_url)
            inputs.append(response['input'])
            txes[response['txhash']] = response['tx']
        except RateLimitException:
            utxos.append(utxo)
            time.sleep(0.3)

    # for utxo in utxos:
    #     try:
    #         response = get_input(utxo)
    #     except RateLimitException:
    #         time.sleep(0.3)
    #         response = get_input(utxo)
    #     inputs.append(response['input'])
    #     txes[response['txhash']] = response['tx']
    # return inputs, txes

def get_input2(utxo, request_url):
    utxo_hash = bytes.fromhex(utxo['txid'])
    utxo_index = int(utxo['output_n'])
    txhash = utxo_hash.hex()
    inputs = [trezorlib.messages.TxInputType(
        prev_hash=b'',
        prev_index=0,
        script_sig=b'',
        sequence=0,
                # prev_hash=bytes.fromhex(vin["txid"]),
                # prev_index=vin["vout"],
                # script_sig=bytes.fromhex(vin["scriptSig"]["hex"]),
                # sequence=0,vin["sequence"],
            )]
    bin_outputs = [trezorlib.messages.TxOutputBinType(
        amount=0,
        script_pubkey=b'',
            # amount=int(Decimal(vout["value"]) * (10**8)),
            # script_pubkey=bytes.fromhex(vout["scriptPubKey"]["hex"]),
        )]
    tx = trezorlib.messages.TransactionType(
        # version=json_dict["version"],
        version=1,
        lock_time=0,
        # lock_time=json_dict.get("locktime", 0),
        inputs=inputs, #[make_input(vin) for vin in json_dict["vin"]],
        bin_outputs=bin_outputs, #[make_bin_output(vout) for vout in json_dict["vout"]],
    )

    # from_address = tx_json["vout"][utxo_index]["scriptPubKey"]["address"]
    # amount = tx.bin_outputs[utxo_index].amount
    amount = utxo['value']
    # echo(f"From address: {from_address} txid:{tx_json['txid']} amount:{amount}")
    address_n = trezorlib.tools.parse_path("m/44'/0'/0'/0/0")

    # reported_type = tx_json["vout"][utxo_index]["scriptPubKey"].get("type") # TODO
    # script_type = BITCOIN_CORE_INPUT_TYPES[reported_type]
    script_type = trezorlib.messages.InputScriptType.SPENDADDRESS # TODO
    sequence = 0xFFFFFFFD

    new_input = trezorlib.messages.TxInputType(
        address_n=address_n,
        prev_hash=utxo_hash,
        prev_index=utxo_index,
        amount=amount,
        script_type=script_type,
        sequence=sequence,
    )

    return {'input': new_input, 'tx': tx, 'txhash': txhash}
'''

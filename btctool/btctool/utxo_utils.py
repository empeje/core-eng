import datetime

import click
import requests
import tqdm


def fetch_utxos(address, utxo_fetch_limit):
    utxos = []
    skip = 0
    amount = 0
    request_limit = 5_000
    count = 0
    with tqdm.tqdm(total=utxo_fetch_limit, desc=f'Fetching UTXOs from the Bitgo API {utxo_fetch_limit=:,}') as bar:
        while True:
            # click.echo(f'request {skip=:,} {limit=:,}')
            url = f'https://www.bitgo.com/api/v1/address/{address}/unspents?limit={request_limit}&skip={skip}'
            # click.echo(url)
            res = requests.get(url)
            if not res.ok:
                if request_limit > 1:
                    request_limit = request_limit // 2
                    continue
                else:
                    # click.echo(f'Failed to fetch all utxos: {res.json()}')
                    raise Exception(f'Failed to fetch all utxos: {res.json()}')

            res_json = res.json()
            skip = res_json['start'] + res_json['count']

            for utxo in res_json['unspents']:
                amount += utxo['value']
                utxos.append({
                    'address': utxo['address'],
                    'txid': utxo['tx_hash'],
                    'confirmations': utxo['confirmations'],
                    'output_n': utxo['tx_output_n'],
                    'input_n': 0,
                    'block_height': int(utxo['blockHeight']) if utxo['blockHeight'] else None,
                    'fee': None,
                    'size': 0,
                    'value': utxo['value'],
                    'script': utxo['script'],
                    # 'date': datetime.datetime.strptime(utxo['date'], "%Y-%m-%dT%H:%M:%S.%fZ")
                })

            new_count = len(utxos)
            if new_count > count:
                bar.set_description(f'Fetched {len(utxos):,} UTXOs from the BitGo API {amount=:,}')
                bar.update(new_count-count)
                count = new_count

            if len(utxos) >= utxo_fetch_limit or res_json['count'] < request_limit:
                break

    utxos = utxos[:utxo_fetch_limit]
    return utxos


'''
import dataclasses
import utils
import multiprocessing
import multiprocessing.queues
import time
from typing import Optional

@utils.ignore_keyboard_interrupt
def leader_worker(request_queue, response_queue, utxos_list):
    limit = 5_000
    # limit = 500
    request_queue.put(Work(page=0, skip=0, limit=limit))
    requested_pages = set([0])
    response_pages = set()
    response_ok_pages = set()
    furthest_ok_work = None
    nearest_fail_work = None
    found_last_page = False
    last_log_time = 0
    utxos = set()
    while True:
        loop = len(requested_pages) - len(response_pages) > 0
        if len(response_pages) > 5:
            loop = False

        t = time.time()
        if t > last_log_time + 3 or not loop:
            last_log_time = t
            amount = sum([u['value'] for u in utxos_list])
            click.echo(
                f'utxos: {len(utxos):,} amount: {amount:,} queued_pages: {sorted(requested_pages - response_pages)} requested_pages: {sorted(requested_pages)} response_pages: {sorted(response_pages)} response_ok_pages: {sorted(response_ok_pages)}')

        if not loop:
            break
        try:
            work = response_queue.get(False, 1)
        except multiprocessing.queues.Empty:
            continue

        response_pages.add(work.page)
        if work.response.ok:
            response_ok_pages.add(work.page)
            resp_json = work.response.json()
            work.start = resp_json['start']
            work.count = resp_json['count']
            # click.echo(f'page: {work.page} unspents: {len(resp_json['unspents'])}')
            for utxo_json in resp_json['unspents']:
                utxo = {
                    'address': utxo_json['address'],
                    'txid': utxo_json['tx_hash'],
                    'confirmations': utxo_json['confirmations'],
                    'output_n': utxo_json['tx_output_n'],
                    'input_n': 0,
                    'block_height': int(utxo_json['blockHeight']) if utxo_json['blockHeight'] else None,
                    'fee': None,
                    'size': 0,
                    'value': utxo_json['value'],
                    'script': utxo_json['script'],
                    'date': datetime.datetime.strptime(utxo_json['date'], "%Y-%m-%dT%H:%M:%S.%fZ")
                }
                utxo_key = (utxo_json['tx_hash'], utxo_json['tx_output_n'])
                if utxo_key in utxos:
                    raise Exception('duplicate utxo')
                utxos.add(utxo_key)
                utxos_list.append(utxo)

            if furthest_ok_work is None or work.page > furthest_ok_work.page:
                furthest_ok_work = work
        else:
            click.echo(f'Failed to fetch all utxos: {work.page=:,}')
            if nearest_fail_work is None or work.page < nearest_fail_work.page:
                nearest_fail_work = work

        # if furthest_ok_work.page:
        for page in range(0, furthest_ok_work.page):
            if page not in requested_pages:
                work = Work(page=page, skip=page * limit, limit=limit)
                # click.echo(f'adding work1 {work}')
                request_queue.put(work)
                requested_pages.add(page)

        if furthest_ok_work.count < furthest_ok_work.limit:
            found_last_page = True

        if not found_last_page:
            min_page = furthest_ok_work.page + 1
            if nearest_fail_work is None:
                max_page = 2 * min_page
            else:
                max_page = nearest_fail_work.page - 1

            pages_to_add = max(0, 10 - request_queue.qsize())
            for i in range(1, pages_to_add + 1):
                page = round(min_page + (max_page - min_page) / 2 ** i)
                if page not in requested_pages:
                    skip = page * limit
                    work = Work(page=page, skip=skip, limit=limit)
                    # click.echo(f'adding work2 {work}')
                    request_queue.put(work)
                    requested_pages.add(page)


@utils.ignore_keyboard_interrupt
def follow_worker(request_queue, response_queue, address):
    while True:
        work = request_queue.get()
        try:
            url = f'https://www.bitgo.com/api/v1/address/{address}/unspents?limit={work.limit}&skip={work.skip}'
            work.response = requests.get(url)
            # click.echo(f'fetch {work.skip=:,} {work.limit=:,} {work.response.ok=}')
            response_queue.put(work)
        finally:
            request_queue.task_done()


@utils.ignore_keyboard_interrupt
def counter_worker(request_queue, response_queue, utxos):
    while True:
        click.echo(f"found utxos: {len(utxos):,}")
        time.sleep(1)


@dataclasses.dataclass
class Work:
    page: int
    skip: int
    limit: int

    response: Optional[requests.models.Request] = None
    start: Optional[int] = None
    count: Optional[int] = None


def fetch_utxos_parallel(address):
    manager = multiprocessing.Manager()
    request_queue = manager.Queue()
    response_queue = manager.Queue()
    utxos = manager.list()

    procs = []

    leader = multiprocessing.Process(target=leader_worker, args=[request_queue, response_queue, utxos])
    procs.append(leader)

    # counter = multiprocessing.Process(target=counter_worker, args=[request_queue, response_queue, utxos])
    # procs.append(counter)

    for i in range(20):
        follower = multiprocessing.Process(target=follow_worker, args=[request_queue, response_queue, address])
        procs.append(follower)

    for proc in procs:
        proc.start()

    leader.join()

    for proc in procs:
        proc.terminate()

    return list(utxos)
'''
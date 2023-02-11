import logging

import click

import consolidate
import sign_command

logger = logging.getLogger()
logger.setLevel(logging.DEBUG)


@click.group()
@click.option('--debug/--no-debug', default=False)
def entry_point(debug):
    pass


def main():
    entry_point.add_command(consolidate.cmd_consolidate)
    entry_point.add_command(sign_command.cmd_sign)
    entry_point()


if __name__ == "__main__":
    main()

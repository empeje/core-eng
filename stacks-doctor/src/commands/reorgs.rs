use std::path::Path;

use anyhow::{Context, Result};
use rusqlite::OpenFlags;
use serde::Serialize;
use serde_json::to_string_pretty;

use crate::cli::{BlocksArgs, Network};

#[derive(Serialize)]
struct Item {
    burn_height: i64,
    stacks_block_id: String,
    miner: String,
    stacks_height: i64,
    reorg_depth: i64,
}

#[derive(Serialize)]
struct Response {
    message: String,
    max_reorg_depth: i64,
    max_reorg_blocks_ago: usize,
    data: Vec<Item>,
}

pub fn reorgs(network: Network, db_dir: &Path, args: &BlocksArgs) -> Result<()> {
    let mode = match network {
        Network::Mainnet => "mainnet/",
        Network::Testnet => "xenon/",
    };
    let db_file = db_dir.join(mode).join("chainstate/vm/index.sqlite");
    let conn = rusqlite::Connection::open_with_flags(db_file, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("Could not open database connection")?;

    let mut statement = conn
        .prepare(
            r#"
            SELECT
               block_headers.burn_header_height as burn_height,
               b.index_block_hash as index_block_hash,
               payments.address as miner,
               block_headers.block_height as stacks_height
            FROM staging_blocks as b
            INNER JOIN block_headers ON block_headers.index_block_hash = b.index_block_hash
            INNER JOIN payments ON payments.index_block_hash = b.index_block_hash
            WHERE payments.miner = 1 GROUP BY block_headers.burn_header_height
            ORDER BY burn_height DESC
            LIMIT ?1;
        "#,
        )
        .context("Could not prepare SQL statement")?;

    let mut rows = statement
        .query::<[i64; 1]>([args.blocks as i64 + 1])
        .context("Could not execute query")?;

    let mut data = Vec::new();

    while let Some(row) = rows.next().context("Could not get row")? {
        let burn_height: i64 = row.get(0).context("Could not get burn height")?;
        let stacks_block_id: String = row.get(1).context("Could not get stacks block id")?;
        let miner: String = row.get(2).context("Could not get miner")?;
        let stacks_height: i64 = row.get(3).context("Could not get stacks height")?;

        data.push(Item {
            burn_height,
            stacks_block_id,
            miner,
            stacks_height,
            reorg_depth: 0,
        });
    }

    let mut last_stacks_height = data
        .pop()
        .context("No blocks returned from query")?
        .stacks_height;

    data.iter_mut().rev().for_each(|mut item| {
        item.reorg_depth = (1 + last_stacks_height - item.stacks_height).max(0);
        last_stacks_height = item.stacks_height;
    });

    let max_reorg_depth = data
        .iter()
        .max_by_key(|item| item.reorg_depth)
        .unwrap()
        .reorg_depth;
    let max_reorg_blocks_ago = data
        .iter()
        .enumerate()
        .find(|(_, item)| item.reorg_depth == max_reorg_depth)
        .unwrap()
        .0;

    let res = Response {
        message: format!("Reorg data for last {} blocks", args.blocks),
        max_reorg_depth,
        max_reorg_blocks_ago,
        data,
    };

    println!("{}", to_string_pretty(&res).unwrap());

    Ok(())
}

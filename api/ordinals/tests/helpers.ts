import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { FastifyBaseLogger, FastifyInstance } from 'fastify';
import { IncomingMessage, Server, ServerResponse } from 'http';
import * as fs from 'fs';
import * as path from 'path';
import { PgSqlClient } from '@hirosystems/api-toolkit';

export const ORDINALS_MIGRATIONS_DIR = '../../migrations/ordinals';
export const BRC20_MIGRATIONS_DIR = '../../migrations/ordinals-brc20';

/// Runs SQL migrations based on the Rust `refinery` crate standard.
export async function runMigrations(sql: PgSqlClient, directory: string) {
  const files = fs.readdirSync(directory);
  const sqlFiles = files
    .filter(file => path.extname(file).toLowerCase() === '.sql')
    .map(file => path.join(directory, file))
    .sort((a, b) => {
      const numA = parseInt(a.match(/\d+/)?.toString() || '0', 10);
      const numB = parseInt(b.match(/\d+/)?.toString() || '0', 10);
      return numA - numB;
    });
  for (const sqlFile of sqlFiles) await sql.file(sqlFile);
  return sqlFiles;
}

/// Drops all tables and types from a test DB. Equivalent to a migration rollback, which are
/// unsupported by the `refinery` crate.
export async function clearDb(sql: PgSqlClient) {
  await sql`
    DO $$ DECLARE
      r RECORD;
    BEGIN
      FOR r IN (SELECT tablename FROM pg_tables WHERE schemaname = current_schema()) LOOP
        EXECUTE 'DROP TABLE IF EXISTS ' || quote_ident(r.tablename) || ' CASCADE';
      END LOOP;
    END $$;
  `;
  await sql`
    DO $$ DECLARE
      r RECORD;
    BEGIN
      FOR r IN (SELECT typname FROM pg_type WHERE typtype = 'e' AND typnamespace = (SELECT oid FROM pg_namespace WHERE nspname = current_schema())) LOOP
        EXECUTE 'DROP TYPE IF EXISTS ' || quote_ident(r.typname) || ' CASCADE';
      END LOOP;
    END $$;
  `;
}

export type TestFastifyServer = FastifyInstance<
  Server,
  IncomingMessage,
  ServerResponse,
  FastifyBaseLogger,
  TypeBoxTypeProvider
>;

type TestOrdinalsInscriptionsRow = {
  inscription_id: string;
  ordinal_number: string;
  number: string;
  classic_number: string;
  block_height: string;
  block_hash: string;
  tx_id: string;
  tx_index: number;
  address: string | null;
  mime_type: string;
  content_type: string;
  content_length: number;
  content: string;
  fee: string;
  curse_type: string | null;
  recursive: boolean;
  input_index: number;
  pointer: string | null;
  metadata: string | null;
  metaprotocol: string | null;
  parent: string | null;
  delegate: string | null;
  timestamp: number;
};
async function insertTestInscription(sql: PgSqlClient, row: TestOrdinalsInscriptionsRow) {
  await sql`INSERT INTO inscriptions ${sql(row)}`;
}

type TestOrdinalsLocationsRow = {
  ordinal_number: string;
  block_height: string;
  tx_index: number;
  tx_id: string;
  block_hash: string;
  address: string | null;
  output: string;
  offset: string | null;
  prev_output: string | null;
  prev_offset: string | null;
  value: string | null;
  transfer_type: string;
  timestamp: number;
};
async function insertTestLocation(sql: PgSqlClient, row: TestOrdinalsLocationsRow) {
  await sql`
    INSERT INTO locations ${sql(row)}
    ON CONFLICT (ordinal_number, block_height, tx_index) DO NOTHING
  `;
}

type TestOrdinalsCurrentLocationsRow = {
  ordinal_number: string;
  block_height: string;
  tx_id: string;
  tx_index: number;
  address: string;
  output: string;
  offset: string | null;
};
async function insertTestCurrentLocation(sql: PgSqlClient, row: TestOrdinalsCurrentLocationsRow) {
  await sql`
    INSERT INTO current_locations ${sql(row)}
    ON CONFLICT (ordinal_number) DO UPDATE SET
      block_height = EXCLUDED.block_height,
      tx_id = EXCLUDED.tx_id,
      tx_index = EXCLUDED.tx_index,
      address = EXCLUDED.address,
      output = EXCLUDED.output,
      \"offset\" = EXCLUDED.\"offset\"
  `;
}

type TestOrdinalsSatoshisRow = {
  ordinal_number: string;
  rarity: string;
  coinbase_height: string;
};
async function insertTestSatoshi(sql: PgSqlClient, row: TestOrdinalsSatoshisRow) {
  await sql`
    INSERT INTO satoshis ${sql(row)}
    ON CONFLICT (ordinal_number) DO NOTHING
  `;
}

type TestOrdinalsInscriptionTransfersRow = {
  inscription_id: string;
  number: string;
  ordinal_number: string;
  block_height: string;
  tx_index: number;
  from_block_height: string;
  from_tx_index: number;
  block_transfer_index: number;
};
async function insertTestInscriptionTransfer(
  sql: PgSqlClient,
  row: TestOrdinalsInscriptionTransfersRow
) {
  await sql`INSERT INTO inscription_transfers ${sql(row)}`;
}

type TestOrdinalsCountsByBlockRow = {
  block_height: string;
  block_hash: string;
  inscription_count: number;
  inscription_count_accum: number;
  timestamp: number;
};
async function insertTestCountsByBlock(sql: PgSqlClient, row: TestOrdinalsCountsByBlockRow) {
  await sql`
    INSERT INTO counts_by_block ${sql(row)}
    ON CONFLICT (block_height) DO UPDATE SET
      inscription_count = counts_by_block.inscription_count + EXCLUDED.inscription_count,
      inscription_count_accum = counts_by_block.inscription_count_accum + EXCLUDED.inscription_count_accum
  `;
}

export type TestOrdinalsInscriptionRecursionsRow = {
  inscription_id: string;
  ref_inscription_id: string;
};
export async function insertTestInscriptionRecursion(
  sql: PgSqlClient,
  row: TestOrdinalsInscriptionRecursionsRow
) {
  await sql`INSERT INTO inscription_recursions ${sql(row)}`;
}

export type TestOrdinalsInscriptionReveal = TestOrdinalsInscriptionsRow &
  TestOrdinalsLocationsRow &
  TestOrdinalsSatoshisRow &
  TestOrdinalsCurrentLocationsRow;
export async function inscriptionReveal(sql: PgSqlClient, reveal: TestOrdinalsInscriptionReveal) {
  await insertTestSatoshi(sql, {
    ordinal_number: reveal.ordinal_number,
    rarity: reveal.rarity,
    coinbase_height: reveal.coinbase_height,
  });
  await insertTestInscription(sql, {
    inscription_id: reveal.inscription_id,
    ordinal_number: reveal.ordinal_number,
    number: reveal.number,
    classic_number: reveal.classic_number,
    block_height: reveal.block_height,
    block_hash: reveal.block_hash,
    tx_id: reveal.tx_id,
    tx_index: reveal.tx_index,
    address: reveal.address,
    mime_type: reveal.mime_type,
    content_type: reveal.content_type,
    content_length: reveal.content_length,
    content: reveal.content,
    fee: reveal.fee,
    curse_type: reveal.curse_type,
    recursive: reveal.recursive,
    input_index: reveal.input_index,
    pointer: reveal.pointer,
    metadata: reveal.metadata,
    metaprotocol: reveal.metaprotocol,
    parent: reveal.parent,
    delegate: reveal.delegate,
    timestamp: reveal.timestamp,
  });
  await insertTestLocation(sql, {
    ordinal_number: reveal.ordinal_number,
    block_height: reveal.block_height,
    tx_index: reveal.tx_index,
    tx_id: reveal.tx_id,
    block_hash: reveal.block_hash,
    address: reveal.address,
    output: reveal.output,
    offset: reveal.offset,
    prev_output: reveal.prev_output,
    prev_offset: reveal.prev_offset,
    value: reveal.value,
    transfer_type: reveal.transfer_type,
    timestamp: reveal.timestamp,
  });
  await insertTestCurrentLocation(sql, {
    ordinal_number: reveal.ordinal_number,
    block_height: reveal.block_height,
    tx_index: reveal.tx_index,
    tx_id: reveal.tx_id,
    address: reveal.address,
    output: reveal.output,
    offset: reveal.offset,
  });
  await insertTestCountsByBlock(sql, {
    block_height: reveal.block_height,
    block_hash: reveal.block_hash,
    inscription_count: 1,
    inscription_count_accum: 1,
    timestamp: reveal.timestamp,
  });
  await sql`
    INSERT INTO counts_by_mime_type ${sql({ mime_type: reveal.mime_type, count: 1 })}
    ON CONFLICT (mime_type) DO UPDATE SET
      count = counts_by_mime_type.count + EXCLUDED.count
  `;
  await sql`
    INSERT INTO counts_by_sat_rarity ${sql({ rarity: reveal.rarity, count: 1 })}
    ON CONFLICT (rarity) DO UPDATE SET
      count = counts_by_sat_rarity.count + EXCLUDED.count
  `;
  await sql`
    INSERT INTO counts_by_type ${sql({
      type: parseInt(reveal.classic_number) >= 0 ? 'blessed' : 'cursed',
      count: 1,
    })}
    ON CONFLICT (type) DO UPDATE SET
      count = counts_by_type.count + EXCLUDED.count
  `;
  await sql`
    INSERT INTO counts_by_address ${sql({ address: reveal.address, count: 1 })}
    ON CONFLICT (address) DO UPDATE SET
      count = counts_by_address.count + EXCLUDED.count
  `;
  await sql`
    INSERT INTO counts_by_genesis_address ${sql({ address: reveal.address, count: 1 })}
    ON CONFLICT (address) DO UPDATE SET
      count = counts_by_genesis_address.count + EXCLUDED.count
  `;
  await sql`
    INSERT INTO counts_by_recursive ${sql({ recursive: reveal.recursive, count: 1 })}
    ON CONFLICT (recursive) DO UPDATE SET
      count = counts_by_recursive.count + EXCLUDED.count
  `;
}

export type TestOrdinalsInscriptionTransfer = TestOrdinalsLocationsRow &
  TestOrdinalsCurrentLocationsRow &
  TestOrdinalsInscriptionTransfersRow;
export async function inscriptionTransfer(
  sql: PgSqlClient,
  transfer: TestOrdinalsInscriptionTransfer
) {
  await insertTestLocation(sql, {
    ordinal_number: transfer.ordinal_number,
    block_height: transfer.block_height,
    tx_index: transfer.tx_index,
    tx_id: transfer.tx_id,
    block_hash: transfer.block_hash,
    address: transfer.address,
    output: transfer.output,
    offset: transfer.offset,
    prev_output: transfer.prev_output,
    prev_offset: transfer.prev_offset,
    value: transfer.value,
    transfer_type: transfer.transfer_type,
    timestamp: transfer.timestamp,
  });
  await insertTestCurrentLocation(sql, {
    ordinal_number: transfer.ordinal_number,
    block_height: transfer.block_height,
    tx_index: transfer.tx_index,
    tx_id: transfer.tx_id,
    address: transfer.address,
    output: transfer.output,
    offset: transfer.offset,
  });
  await insertTestInscriptionTransfer(sql, {
    inscription_id: transfer.inscription_id,
    number: transfer.number,
    ordinal_number: transfer.ordinal_number,
    block_height: transfer.block_height,
    tx_index: transfer.tx_index,
    from_block_height: transfer.from_block_height,
    from_tx_index: transfer.from_tx_index,
    block_transfer_index: transfer.block_transfer_index,
  });
}

/** Generate a random hash like string for testing */
export const randomHash = () =>
  [...Array(64)].map(() => Math.floor(Math.random() * 16).toString(16)).join('');

/** Generator for incrementing numbers */
export function* incrementing(
  start: number = 0,
  step: number = 1
): Generator<number, number, 'next'> {
  let current = start;

  while (true) {
    yield current;
    current += step;
  }
}

// export async function deployAndMintPEPE(db: PgStore, address: string) {
//   await db.updateInscriptions(
//     new TestChainhookPayloadBuilder()
//       .apply()
//       .block({
//         height: BRC20_GENESIS_BLOCK,
//         hash: '00000000000000000002a90330a99f67e3f01eb2ce070b45930581e82fb7a91d',
//       })
//       .transaction({
//         hash: '38c46a8bf7ec90bc7f6b797e7dc84baa97f4e5fd4286b92fe1b50176d03b18dc',
//       })
//       .brc20(
//         {
//           deploy: {
//             tick: 'pepe',
//             max: '250000',
//             dec: '18',
//             lim: '250000',
//             inscription_id: '38c46a8bf7ec90bc7f6b797e7dc84baa97f4e5fd4286b92fe1b50176d03b18dci0',
//             address,
//             self_mint: false,
//           },
//         },
//         { inscription_number: 0 }
//       )
//       .build()
//   );
//   await db.updateInscriptions(
//     new TestChainhookPayloadBuilder()
//       .apply()
//       .block({
//         height: BRC20_GENESIS_BLOCK + 1,
//         hash: '0000000000000000000098d8f2663891d439f6bb7de230d4e9f6bcc2e85452bf',
//       })
//       .transaction({
//         hash: '3b55f624eaa4f8de6c42e0c490176b67123a83094384f658611faf7bfb85dd0f',
//       })
//       .brc20(
//         {
//           mint: {
//             tick: 'pepe',
//             amt: '10000',
//             inscription_id: '3b55f624eaa4f8de6c42e0c490176b67123a83094384f658611faf7bfb85dd0fi0',
//             address,
//           },
//         },
//         { inscription_number: 1 }
//       )
//       .build()
//   );
// }

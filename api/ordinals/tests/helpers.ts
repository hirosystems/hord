import { TypeBoxTypeProvider } from '@fastify/type-provider-typebox';
import { FastifyBaseLogger, FastifyInstance } from 'fastify';
import { IncomingMessage, Server, ServerResponse } from 'http';
import * as fs from 'fs';
import * as path from 'path';
import { PgSqlClient } from '@hirosystems/api-toolkit';

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
  for (const sqlFile of sqlFiles) {
    await sql.file(sqlFile);
  }
  return sqlFiles;
}

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
  await sql`INSERT INTO locations ${sql(row)}`;
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
  await sql`INSERT INTO satoshis ${sql(row)}`;
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

// export class TestChainhookPayloadBuilder {
//   private payload: BitcoinPayload = {
//     apply: [],
//     rollback: [],
//     chainhook: {
//       uuid: 'test',
//       predicate: {
//         scope: 'ordinals_protocol',
//         operation: 'inscription_feed',
//         meta_protocols: ['brc-20'],
//       },
//       is_streaming_blocks: false,
//     },
//   };
//   private action: 'apply' | 'rollback' = 'apply';
//   private get lastBlock(): BitcoinEvent {
//     return this.payload[this.action][this.payload[this.action].length - 1] as BitcoinEvent;
//   }
//   private get lastBlockTx(): BitcoinTransaction {
//     return this.lastBlock.transactions[this.lastBlock.transactions.length - 1];
//   }
//   private txIndex = 0;

//   streamingBlocks(streaming: boolean): this {
//     this.payload.chainhook.is_streaming_blocks = streaming;
//     return this;
//   }

//   apply(): this {
//     this.action = 'apply';
//     return this;
//   }

//   rollback(): this {
//     this.action = 'rollback';
//     return this;
//   }

//   block(args: { height: number; hash?: string; timestamp?: number }): this {
//     this.payload[this.action].push({
//       block_identifier: {
//         index: args.height,
//         hash: args.hash ?? '0x163de66dc9c0949905bfe8e148bde04600223cf88d19f26fdbeba1d6e6fa0f88',
//       },
//       parent_block_identifier: {
//         index: args.height - 1,
//         hash: '0x117374e7078440835a744b6b1b13dd2c48c4eff8c58dde07162241a8f15d1e03',
//       },
//       timestamp: args.timestamp ?? 1677803510,
//       transactions: [],
//       metadata: {},
//     } as BitcoinEvent);
//     return this;
//   }

//   transaction(args: { hash: string }): this {
//     this.lastBlock.transactions.push({
//       transaction_identifier: {
//         hash: args.hash,
//       },
//       operations: [],
//       metadata: {
//         ordinal_operations: [],
//         proof: null,
//         index: this.txIndex++,
//       },
//     });
//     return this;
//   }

//   inscriptionRevealed(args: BitcoinInscriptionRevealed): this {
//     this.lastBlockTx.metadata.ordinal_operations.push({ inscription_revealed: args });
//     return this;
//   }

//   inscriptionTransferred(args: BitcoinInscriptionTransferred): this {
//     this.lastBlockTx.metadata.ordinal_operations.push({ inscription_transferred: args });
//     return this;
//   }

//   brc20(
//     args: BitcoinBrc20Operation,
//     opts: { inscription_number: number; ordinal_number?: number }
//   ): this {
//     this.lastBlockTx.metadata.brc20_operation = args;
//     if ('transfer_send' in args) {
//       this.lastBlockTx.metadata.ordinal_operations.push({
//         inscription_transferred: {
//           ordinal_number: opts.ordinal_number ?? opts.inscription_number,
//           destination: {
//             type: 'transferred',
//             value: args.transfer_send.receiver_address,
//           },
//           satpoint_pre_transfer: `${args.transfer_send.inscription_id.split('i')[0]}:0:0`,
//           satpoint_post_transfer: `${this.lastBlockTx.transaction_identifier.hash}:0:0`,
//           post_transfer_output_value: null,
//           tx_index: 0,
//         },
//       });
//     } else {
//       let inscription_id = '';
//       let inscriber_address = '';
//       if ('deploy' in args) {
//         inscription_id = args.deploy.inscription_id;
//         inscriber_address = args.deploy.address;
//       } else if ('mint' in args) {
//         inscription_id = args.mint.inscription_id;
//         inscriber_address = args.mint.address;
//       } else {
//         inscription_id = args.transfer.inscription_id;
//         inscriber_address = args.transfer.address;
//       }
//       this.lastBlockTx.metadata.ordinal_operations.push({
//         inscription_revealed: {
//           content_bytes: `0x101010`,
//           content_type: 'text/plain;charset=utf-8',
//           content_length: 3,
//           inscription_number: {
//             jubilee: opts.inscription_number,
//             classic: opts.inscription_number,
//           },
//           inscription_fee: 2000,
//           inscription_id,
//           inscription_output_value: 10000,
//           inscriber_address,
//           ordinal_number: opts.ordinal_number ?? opts.inscription_number,
//           ordinal_block_height: 0,
//           ordinal_offset: 0,
//           satpoint_post_inscription: `${inscription_id.split('i')[0]}:0:0`,
//           inscription_input_index: 0,
//           transfers_pre_inscription: 0,
//           tx_index: 0,
//           curse_type: null,
//           inscription_pointer: null,
//           delegate: null,
//           metaprotocol: null,
//           metadata: undefined,
//           parent: null,
//         },
//       });
//     }
//     return this;
//   }

//   build(): BitcoinPayload {
//     return this.payload;
//   }
// }

// export function rollBack(payload: BitcoinPayload) {
//   return {
//     ...payload,
//     apply: [],
//     rollback: payload.apply,
//   };
// }

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

export const BRC20_GENESIS_BLOCK = 779832;
export const BRC20_SELF_MINT_ACTIVATION_BLOCK = 837090;

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

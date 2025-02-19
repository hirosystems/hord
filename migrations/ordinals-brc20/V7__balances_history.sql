CREATE TABLE balances_history (
  ticker TEXT NOT NULL,
  address TEXT NOT NULL,
  block_height NUMERIC NOT NULL,
  avail_balance NUMERIC NOT NULL,
  trans_balance NUMERIC NOT NULL,
  total_balance NUMERIC NOT NULL
);
ALTER TABLE balances_history ADD PRIMARY KEY (address, block_height, ticker);
ALTER TABLE balances_history ADD CONSTRAINT balances_history_ticker_fk FOREIGN KEY(ticker) REFERENCES tokens(ticker) ON DELETE CASCADE;
CREATE INDEX balances_history_block_height_index ON balances_history (block_height DESC);

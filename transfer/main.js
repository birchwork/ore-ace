const task = require("tasuku");
const { PublicKey } = require("@solana/web3.js");
const { Adapter } = require("./utils/adapters");
const { readAndValidatePrivateKeys } = require("./utils/readAndValidate");
const { initializeConnection, initializeKeypair } = require("./utils/initiali");
const { config } = require("./config");

async function main() {
  const list = await readAndValidatePrivateKeys();
  await task("Starting Token Transfer Process...", async ({ setTitle }) => {
    for (const key of list) {
      const connection = initializeConnection();
      const fromKeypair = await initializeKeypair(key);

      const wallet = new Adapter(
        connection,
        fromKeypair,
        new PublicKey(config.spltoken),
        new PublicKey(config.beneficiary)
      );

      await wallet.transfer();
    }
    setTitle("Done.");
  });
}

main();

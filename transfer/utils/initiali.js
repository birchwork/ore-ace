const bs58 = require("bs58");
const { config } = require("../config");
const { Connection, Keypair } = require("@solana/web3.js");

async function initializeKeypair(privateKey) {
  try {
    const keypair = Keypair.fromSecretKey(
      new Uint8Array(bs58.decode(privateKey))
    );

    return keypair;
  } catch (err) {
    console.log("Error:", err);
  }
}

function initializeConnection() {
  const rpcUrl = config.rpcUrl;
  const connection = new Connection(rpcUrl, {
    commitment: "confirmed",
    wsEndpoint: config.wsEndpoint,
  });

  return connection;
}

module.exports = {
  initializeKeypair,
  initializeConnection,
};

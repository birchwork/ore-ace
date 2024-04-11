const fs = require("fs");
const bs58 = require("bs58");
const { Keypair } = require("@solana/web3.js");

async function readAndValidatePrivateKeys() {
  try {
    // Read the content of the file asynchronously using UTF-8 encoding
    const fileContent = await fs.promises.readFile("PrivateKeys.txt", {
      encoding: "utf-8",
    });

    // Split the file content into an array of lines
    const lines = fileContent.split("\n");

    // Array to store valid private keys
    const validPrivateKeys = [];

    for (let line of lines) {
      const key = line.trim();

      try {
        const keypair = Keypair.fromSecretKey(bs58.decode(key));
        // If successful, add to the valid keys array
        validPrivateKeys.push(key);
      } catch (error) {
        // Log invalid keys along with the error message
        console.log(`Invalid key detected: ${line}`, error.message);
      }
    }
    return validPrivateKeys;
  } catch (error) {
    console.error("Error reading file:", error);
    return [];
  }
}

module.exports = {
  readAndValidatePrivateKeys,
};

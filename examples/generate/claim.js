const fs = require("fs");
const path = require("path");

/**
 * Generates a YAML configuration file for mining services based on the provided parameters.
 * @param {string} filename - The path to the file containing private key.
 * @param {string} baseRPC - The base RPC URL, mandatory for operation.
 * @param {Object} options - An object containing optional parameters:
 *   - {string} [jitoFee] - The fee for using JitoTips.
 *   - {boolean} [jitoEnable=false] - Flag to enable or disable JitoTips.
 *   - {string} [priorityFee="0"] - The priority fee per transaction.
 *   - {string} [threads="1"] - The number of threads for mining.
 */
function generateYAML(filename, baseRPC, name, options = {}) {
  // Destructuring the options with default values
  const {
    jitoFee,
    jitoEnable = false,
    priorityFee = "0",
    threads = "1",
  } = options;

  fs.readFile(filename, "utf8", (err, data) => {
    if (err) {
      console.error("Error reading the file:", err);
      return;
    }

    // Splitting the input file by lines and trimming whitespace
    const keys = data
      .trim()
      .split("\n")
      .map((key) => key.trim());

    let yamlContent = 'version: "0.1"\n\nservices:\n';

    // Iterating over each key to generate service entries
    keys.forEach((key, index) => {
      const serviceName = `claim-${name}-${String(index + 1).padStart(3, "0")}`;
      const containerName = `claim-${name}-${String(index + 1).padStart(3, "0")}`;

      // Constructing the YAML content dynamically based on provided parameters
      yamlContent += `  ${serviceName}:\n`;
      yamlContent += `    container_name: ${containerName}\n`;
      yamlContent += `    image: ghcr.io/birchwork/ore-ace:latest\n`;
      yamlContent += `    command:\n`;
      if (jitoEnable) {
        yamlContent += `      - "--jito-enable"\n`;
        if (jitoFee) {
          yamlContent += `      - "--jito-fee"\n      - "${jitoFee}"\n`;
        }
      }
      yamlContent += `      - "--rpc"\n      - "${baseRPC}"\n`;
      yamlContent += `      - "--keypair"\n      - "${key}"\n`;
      yamlContent += `      - "claim"\n`;
      yamlContent += "    restart: always\n\n";
    });

    // Writing the generated YAML content to an output file
    fs.writeFile("output-mine.yaml", yamlContent, "utf8", (writeErr) => {
      if (writeErr) {
        console.error("Error writing the YAML file:", writeErr);
        return;
      }
      console.log("YAML file has been generated successfully.");
    });
  });
}

// Example usage of the function
const filename = "./file.txt"; // Replace with the path to your private key file
const baseRPC = "https://example-rpc-url.com"; // Replace with your base RPC URL
const name = "group-1"; // Group name
generateYAML(filename, baseRPC, name, {
  jitoEnable: true,
  jitoFee: "10000",
});

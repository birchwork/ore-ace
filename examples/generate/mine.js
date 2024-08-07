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
function generateYAML(filename, baseRPC, name, options) {
  // Destructuring the options with default values
  const { threads,fee } = options;

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
      const serviceName = `mine-${name}-${String(index + 1).padStart(3, "0")}`;
      const containerName = `mine-${name}-${String(index + 1).padStart(
        3,
        "0"
      )}`;

      // Constructing the YAML content dynamically based on provided parameters
      yamlContent += `  ${serviceName}:\n`;
      yamlContent += `    container_name: ${containerName}\n`;
      yamlContent += `    image: ghcr.io/birchwork/ore-ace:v0.6\n`;
      yamlContent += `    command:\n`;
      yamlContent += `      - "--rpc"\n      - "${baseRPC}"\n`;
      yamlContent += `      - "--private-key"\n      - "${key}"\n`;
      yamlContent += `      - "--priority-fee"\n      - "${fee}"\n`;
      yamlContent += `      - "mine"\n`;
      if (threads !== "1") {
        yamlContent += `      - "--threads"\n      - "${threads}"\n`;
      }
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
  threads: 8,
  fee: 5000,
});

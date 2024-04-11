async function getNumberDecimals(mintAddress, connection) {
  try {
    const info = await connection.getParsedAccountInfo(mintAddress);
    const decimals = (info.value?.data).parsed.info.decimals;
    return decimals;
  } catch (err) {
    console.log("Error:", err);
  }
}

module.exports = {
  getNumberDecimals,
};

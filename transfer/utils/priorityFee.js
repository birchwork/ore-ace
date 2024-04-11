async function getPriorityFee() {
    const url = "https://quicknode.com/_gas-tracker?slug=solana";

    try {
        // Fetch data from the provided URL using the GET method.
        const response = await fetch(url, {
            method: 'GET',
            headers: { 'Accept': 'application/json' }
        });

        if (!response.ok) {
            throw new Error(`HTTP error! status: ${response.status}`);
        }

        const data = await response.json();

        // Destructure and extract percentile values from the data.
        const { '25': p25, '50': p50, '75': p75 } = data.sol.per_transaction.percentiles;

        // Calculate the minimum and maximum range based on the percentile values.
        const minRange = (p50 / 2) + p50; // Example calculation for minimum range.
        const maxRange = (p25 / 5) + p75; // Example calculation for maximum range.

        // Generate a random value within the calculated range.
        const calculatedValue = Math.random() * (maxRange - minRange) + minRange;

        return calculatedValue;

    } catch (error) {
        console.error("Fetching Gas Tracker Data failed:", error);
        return null;
    }
}


module.exports = { getPriorityFee }; 
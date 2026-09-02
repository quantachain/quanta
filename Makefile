.PHONY: testnet clean-testnet logs

testnet:
	@echo "Starting Quanta Local Testnet (4 Nodes)..."
	@mkdir -p testnet_data/db1 testnet_data/db2 testnet_data/db3 testnet_data/db4
	sudo docker-compose -f docker-compose.testnet.yml up -d --build
	@echo "Testnet is running in the background."
	@echo "To view logs side-by-side, run: make logs"

clean-testnet:
	@echo "Stopping Quanta Local Testnet and wiping data..."
	sudo docker-compose -f docker-compose.testnet.yml down -v
	sudo rm -rf testnet_data
	@echo "Testnet cleaned."

logs:
	sudo docker-compose -f docker-compose.testnet.yml logs -f

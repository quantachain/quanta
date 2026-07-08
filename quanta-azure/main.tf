terraform {
  required_providers {
    azurerm = {
      source  = "hashicorp/azurerm"
      version = "~> 3.0"
    }
  }
}

provider "azurerm" {
  features {}
}

variable "node_count" {
  description = "Number of validator nodes to spawn for the Flood Testnet"
  default     = 20
}

variable "location" {
  default = "East US"
}

variable "ssh_public_key" {
  description = "Public SSH key for the VMs"
  type        = string
}

resource "azurerm_resource_group" "testnet" {
  name     = "quanta-flood-testnet-rg"
  location = var.location
}

resource "azurerm_virtual_network" "testnet_vnet" {
  name                = "quanta-testnet-vnet"
  address_space       = ["10.0.0.0/16"]
  location            = azurerm_resource_group.testnet.location
  resource_group_name = azurerm_resource_group.testnet.name
}

resource "azurerm_subnet" "testnet_subnet" {
  name                 = "quanta-testnet-subnet"
  resource_group_name  = azurerm_resource_group.testnet.name
  virtual_network_name = azurerm_virtual_network.testnet_vnet.name
  address_prefixes     = ["10.0.1.0/24"]
}

resource "azurerm_public_ip" "node_ip" {
  count               = var.node_count
  name                = "quanta-node-ip-${count.index}"
  location            = azurerm_resource_group.testnet.location
  resource_group_name = azurerm_resource_group.testnet.name
  allocation_method   = "Dynamic"
}

resource "azurerm_network_security_group" "testnet_nsg" {
  name                = "quanta-testnet-nsg"
  location            = azurerm_resource_group.testnet.location
  resource_group_name = azurerm_resource_group.testnet.name

  security_rule {
    name                       = "SSH"
    priority                   = 1001
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "22"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  security_rule {
    name                       = "Quanta-P2P"
    priority                   = 1002
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "8333"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }

  security_rule {
    name                       = "Quanta-RPC"
    priority                   = 1003
    direction                  = "Inbound"
    access                     = "Allow"
    protocol                   = "Tcp"
    source_port_range          = "*"
    destination_port_range     = "8332"
    source_address_prefix      = "*"
    destination_address_prefix = "*"
  }
}

resource "azurerm_network_interface" "node_nic" {
  count               = var.node_count
  name                = "quanta-node-nic-${count.index}"
  location            = azurerm_resource_group.testnet.location
  resource_group_name = azurerm_resource_group.testnet.name

  ip_configuration {
    name                          = "internal"
    subnet_id                     = azurerm_subnet.testnet_subnet.id
    private_ip_address_allocation = "Dynamic"
    public_ip_address_id          = azurerm_public_ip.node_ip[count.index].id
  }
}

resource "azurerm_network_interface_security_group_association" "node_nsg_assoc" {
  count                     = var.node_count
  network_interface_id      = azurerm_network_interface.node_nic[count.index].id
  network_security_group_id = azurerm_network_security_group.testnet_nsg.id
}

resource "azurerm_linux_virtual_machine" "node_vm" {
  count               = var.node_count
  name                = "quanta-node-${count.index}"
  resource_group_name = azurerm_resource_group.testnet.name
  location            = azurerm_resource_group.testnet.location
  size                = "Standard_B2s"
  admin_username      = "quantaadmin"

  network_interface_ids = [
    azurerm_network_interface.node_nic[count.index].id,
  ]

  admin_ssh_key {
    username   = "quantaadmin"
    public_key = var.ssh_public_key
  }

  os_disk {
    caching              = "ReadWrite"
    storage_account_type = "Standard_LRS"
  }

  source_image_reference {
    publisher = "Canonical"
    offer     = "0001-com-ubuntu-server-jammy"
    sku       = "22_04-lts"
    version   = "latest"
  }
}

resource "local_file" "ansible_inventory" {
  content = templatefile("${path.module}/inventory.tmpl", {
    ips = azurerm_linux_virtual_machine.node_vm[*].public_ip_address
  })
  filename = "${path.module}/inventory.ini"
}

output "node_ips" {
  value = azurerm_linux_virtual_machine.node_vm[*].public_ip_address
}

PCF Archive Cleaner
Description
PCF Archive Cleaner is a Rust-based utility designed to automatically delete archived projects from the Pentest Collaboration Framework (PCF). This tool is specifically tailored for version 1.5.0 of PCF and aims to assist cybersecurity professionals in managing data protection requirements.
Key Features

Automated Cleaning: Runs as a cron job on the server, automatically cleaning archived projects.
Configurable: Uses a YAML configuration file, allowing easy updates to database structure and file deletion patterns without recompiling.
Comprehensive Deletion: Removes data from multiple related tables and associated files.
Logging: Maintains detailed logs of all operations for auditing purposes.

Configuration
The tool uses a config.yaml file to define the database structure and file paths. It supports a hierarchical workflow structure, allowing for the deletion of related data across multiple tables. The configuration includes:

Database tables and their relationships
Columns to query for deletion
File paths for associated documents

Supported Tables
The cleaner supports deletion from the following PCF tables:
Projects, Files, Issues, PoC, Chats, Messages, Credentials, Hosts, Hostnames, Logs, NetworkPaths, Networks, Notes, Ports, Tasks, tool_sniffer_http_info, tool_sniffer_http_data
Usage

Set up the configuration file to match your PCF installation.
Schedule the program to run as a cron job on your server.
The cleaner will automatically remove archived projects and their associated data.

Benefits

Helps maintain data hygiene in long-running PCF installations
Assists in complying with data protection regulations
Reduces database bloat and improves performance
Customizable to fit specific organizational needs

Caution
Always backup your PCF database before running this tool, especially when first setting it up or after making configuration changes.
Contribution
Contributions, issues, and feature requests are welcome. Feel free to check issues page if you want to contribute.
License

[INSERT_LICENSE_HERE]

Disclaimer
This tool is provided as-is. Users are responsible for ensuring it meets their specific data protection and security requirements.

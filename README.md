# Load Balancer in Rust

This project is a load balancer implemented in Rust. It is designed to efficiently distribute incoming network traffic across multiple backend servers, ensuring high availability and reliability.

## Features
- **Efficient Traffic Distribution**: Balances traffic across multiple backends.
- **Geo-based Routing**: Supports routing based on geographic location.
- **Configurable**: Easily customizable via a configuration file.
- **High Performance**: Built with Rust for speed and safety.

## Getting Started

### Prerequisites
- Rust (latest stable version) installed. You can install Rust using [rustup](https://rustup.rs/).

### Building the Project
1. Clone the repository:
   ```bash
   git clone https://github.com/utfunderscore/loadbalancer-rs.git
   cd loadbalancer-rs
   ```
2. Build the project using Cargo:
   ```bash
   cargo build --release
   ```

### Running the Load Balancer
1. Ensure you have a valid `config.yml` file in the root directory. This file contains the configuration for the load balancer.
2. Run the application:
   ```bash
   cargo run --release
   ```

### Configuration
The `config.yml` file allows you to customize the behavior of the load balancer. Example configuration options include:
- Backend server addresses
- Geo-routing rules
- Connection timeouts

Refer to the `config.rs` file for detailed configuration options.

## Contributing

We welcome contributions! To contribute:
1. Fork the repository.
2. Create a new branch for your feature or bugfix:
   ```bash
   git checkout -b feature-name
   ```
3. Make your changes and commit them:
   ```bash
   git commit -m "Add new feature"
   ```
4. Push your changes to your fork:
   ```bash
   git push origin feature-name
   ```
5. Open a pull request on the main repository.

## License

This project is licensed under the GPLv3 License. See the `LICENSE` file for details.

## Support

If you encounter any issues or have questions, feel free to open an issue on the repository or contact the maintainers.

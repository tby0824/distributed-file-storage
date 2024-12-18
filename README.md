# Distributed File Storage System

## Team Members
- Ziyin Zhang (1010784849) - zyin.zhang@mail.utoronto.ca
- Ziming Liu (1006972171) - zimi.liu@mail.utoronto.ca
- Boyuan Tan (1011579258) - boyuan.tan@mail.utoronto.ca

## Motivation

The exponential growth of data in today's digital age has made efficient and secure distributed storage a critical requirement. While industrial-grade distributed storage systems exist, they often present significant barriers for small teams and individual developers:

1. **High Operational Costs**: Existing solutions often require substantial infrastructure investment
2. **Complex Architecture**: Many systems are designed for enterprise-scale deployment, making them overly complex for smaller teams
3. **Steep Learning Curve**: Current solutions often require extensive knowledge of distributed systems

Furthermore, within the Rust ecosystem, there is a notable gap in lightweight distributed storage solutions. While Rust's safety guarantees and performance characteristics make it ideal for systems programming, most existing distributed storage solutions are implemented in Go, Java, or Python. Our project addresses this gap by providing a simple yet functional implementation that serves both practical and educational purposes.

## Objectives

1. **Accessibility**: Create a lightweight distributed storage system that small teams can easily deploy and maintain
2. **Educational Value**: Demonstrate practical implementation of distributed systems concepts in Rust
3. **Performance**: Leverage Rust's concurrency features for efficient file operations
4. **Reliability**: Ensure data integrity through robust error handling and validation
5. **Scalability**: Support dynamic addition and removal of storage nodes

## Prerequisites

### Core Requirements
- Rust 
- Redis
- Docker
- AWS RDS for PostgreSQL
- libp2p
- SQLx

### Key Dependencies
- **libp2p** (0.42.0)
  - P2P networking foundation
  - Noise protocol for encryption
  - PeerId and multiaddr support
  - Custom protocol implementation

- **Tokio** (1.41.0)
  - Asynchronous runtime
  - Event-driven architecture
  - I/O operations
  - Task scheduling

- **SQLx** (0.8.2)
  - PostgreSQL async driver
  - Runtime type checking
  - Macro-based query building
  - Migration management

- **JWT** (jsonwebtoken 8.1 or later)
  - Authentication tokens
  - Claims verification
  - Token signing
  - Expiration management

- **Redis** (0.27.6)
  - Caching implementation
  - Async operations
  - Connection pooling
  - Multiplexed connections

- **Clap** (4.3.0)
  - Command line argument parsing
  - Interactive CLI support
  - Help text generation

## Features

### 1. Distributed Architecture
- **P2P Communication**: 
  - Built on libp2p for robust peer-to-peer networking
  - Custom protocol implementation for file operations
  - Automatic node discovery and registration
  - Real-time node health monitoring through heartbeats

- **File Distribution**:
  - Intelligent chunk allocation across nodes
  - Parallel upload/download operations
  - Automatic chunk verification and integrity checks
  - Fault tolerance mechanisms

### 2. Database Architecture
Our system uses a carefully designed PostgreSQL database (AWS RDS) with five core tables:

- **users**: Manages user authentication and profiles
- **files**: Stores file metadata and ownership information
- **file_chunks**: Tracks individual file chunks and their properties
- **nodes**: Maintains node status and health information
- **chunk_nodes**: Maps chunk distribution across nodes

This design ensures:
- Data integrity through referential constraints
- Efficient query performance
- Scalable metadata management
- Reliable tracking of system state

### 3. Security Implementation
- JWT-based authentication with Argon2 password hashing
- Role-based access control
- Secure session management
- Data integrity verification through SHA-256 checksums

### 4. Performance Optimization
- **Redis Caching**:
  - File list caching
  - Session data caching
  - Automatic cache invalidation
  - Concurrent access support

- **Efficient Data Processing**:
  - Parallel chunk processing
  - Asynchronous I/O operations
  - Optimized data transfer protocols

### 5. Docker Integration
Our system utilizes Docker for consistent deployment across environments:
- Multi-node containerized setup
- Automated networking configuration
- Easy scaling capabilities
- Environment-based configuration

## User's Guide

### Reproducibility Guide

#### Environment Setup

1. Install Rust:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

2. Install Redis:
```bash
# Ubuntu
sudo apt-get update
sudo apt-get install redis-server

# macOS
brew install redis
```

3. Start Redis:
```bash
sudo service redis-server start  # Ubuntu
brew services start redis        # macOS
```

4. Set Environment Variables:
```bash
export DATABASE_URL=postgres://postgres:123456tby@distributed-file-storage.chymoymoc9or.us-east-2.rds.amazonaws.com:5432/dfs_db
export JWT_SECRET=supersecretjwtkey
```

#### Building and Running

1. Clone and Build:
```bash
git clone git@github.com:tby0824/distributed-file-storage.git
cd distributed-file-storage
cargo build --release
```

2. Run Nodes (each in a separate terminal):
# First node (port 9000):
```bash
NODE_ADDRESS="/ip4/127.0.0.1/tcp/9000" cargo run --release
```
# Second node (port 9001):
```bash
NODE_ADDRESS="/ip4/127.0.0.1/tcp/9001" cargo run --release
```
# Third node (port 9002):
```bash
NODE_ADDRESS="/ip4/127.0.0.1/tcp/9002" cargo run --release
```

3. Alternative: Docker Deployment, if you prefer using Docker:
```bash
docker-compose up --build -d

# Verify services are running
docker-compose ps

# View logs
docker-compose logs -f
```
### Basic Commands

#### Node Management
```bash
# View active nodes
list-nodes
```
#### User Management
```bash
# Register new user
register <username> <password> [email]

# Login to system
login <username> <password>

# View current session
whoami

# End session
logout
```

#### File Operations
```bash
# Upload file
upload <file_path>

# Download file
download <file_id>

# List all files
list-files

# Delete file (requires confirmation)
delete <file_id> --confirm

# Batch upload
batch-upload <file1> <file2> ... [--force]

# Rename file
rename-file <old_name> <new_name>
```

## Contributions

### Boyuan Tan
- Database implementation and AWS RDS integration
- Redis caching system
- Docker containerization
- P2P architecture (with Ziming)
- System testing and optimization
- System monitoring
- User authentication and authorization

### Ziming Liu
- File management system implementation
- P2P architecture (with Boyuan)
- Node management and heartbeat system
- Chunk distribution logic
- Performance optimization
- CLI development

### Ziyin Zhang
- Data integrity implementation
- Security features

## Lessons Learned and Concluding Remarks

This project provided valuable insights into distributed systems development using Rust:

### Technical Insights
1. Rust's ownership system proved invaluable for handling distributed state
2. Async/await significantly simplified concurrent operations
3. Type safety helped prevent many common distributed systems bugs

### Implementation Challenges
1. Balancing system complexity with usability
2. Handling network partitions and node failures
3. Ensuring consistent state across distributed components

### Future Enhancements
1. AWS EC2 deployment integration (planned)
2. gRPC implementation for improved communication (planned)

### Educational Value
This project serves as a practical example of:
1. Distributed systems concepts implementation
2. Rust's application in system programming
3. Modern software architecture patterns
4. Real-world security implementation

The experience demonstrated that Rust is an excellent choice for distributed systems development, offering both performance and safety guarantees. While we achieved our core objectives, the project highlighted areas for future enhancement, particularly in deployment automation and inter-service communication.

## Three Minutes Demo Video
🎥 [Watch the Full System Demo (Google Drive)](https://drive.google.com/file/d/1c0MuxlSTqsdwXc62B1M7lYnVTy5yEzqW/view)

## License
This project is licensed under the MIT License - see the LICENSE file for details.

# Quotey Deployment Architecture

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           SLACK WORKSPACE                                    │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐ │
│  │   User      │  │   User      │  │   User      │  │    Slack API        │ │
│  │  (Sales)    │  │  (Manager)  │  │  (Finance)  │  │   (Socket Mode)     │ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────────────────────┘ │
│         │                │                │                    ▲            │
│         │  /quote new    │  /quote status │  Approve quote     │            │
│         │                │                │                    │            │
└─────────┼────────────────┼────────────────┼────────────────────┼────────────┘
          │                │                │                    │
          └────────────────┴────────────────┴────────────────────┘
                                    │
                                    │ WebSocket Connection (Outbound)
                                    │ (No public IP needed!)
                                    ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                           QUOTEY SERVER                                      │
│                                                                              │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │                      quotey-server (Rust)                              │  │
│  │  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  ┌───────────┐  │  │
│  │  │ Slack Socket │  │   Agent      │  │    CPQ       │  │   Flow    │  │  │
│  │  │   Adapter    │◄─┤  Runtime     │◄─┤   Engine     │◄─┤  Engine   │  │  │
│  │  └──────────────┘  └──────────────┘  └──────────────┘  └───────────┘  │  │
│  │          │                │                  │              │          │  │
│  │          └────────────────┴──────────────────┴──────────────┘          │  │
│  │                              │                                         │  │
│  │                              ▼                                         │  │
│  │  ┌────────────────────────────────────────────────────────────────┐   │  │
│  │  │              SQLite Database (quotey.db)                        │   │  │
│  │  │  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │   │  │
│  │  │  │  Quotes  │ │ Products │ │  Rules   │ │  Audit   │           │   │  │
│  │  │  └──────────┘ └──────────┘ └──────────┘ └──────────┘           │   │  │
│  │  └────────────────────────────────────────────────────────────────┘   │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                         │
│  ┌───────────────────────────────────────────────────────────────────────┐  │
│  │  Health Check Server (:8080) - for monitoring/load balancers          │  │
│  │  Endpoints: /health, /ready                                           │  │
│  └───────────────────────────────────────────────────────────────────────┘  │
│                                    │                                         │
└────────────────────────────────────┼─────────────────────────────────────────┘
                                     │
                    ┌────────────────┴────────────────┐
                    │                                 │
                    ▼                                 ▼
┌──────────────────────────────┐      ┌──────────────────────────────┐
│      LLM Provider            │      │      CRM Integration         │
│  ┌────────────────────────┐  │      │  ┌────────────────────────┐  │
│  │  Option 1: Ollama      │  │      │  │  Option 1: Stub (CSV)  │  │
│  │  (Local, Free)         │  │      │  │  (Demo data)           │  │
│  └────────────────────────┘  │      │  └────────────────────────┘  │
│  ┌────────────────────────┐  │      │  ┌────────────────────────┐  │
│  │  Option 2: OpenAI API  │  │      │  │  Option 2: Composio    │  │
│  │  (Cloud, Paid)         │  │      │  │  (HubSpot/Salesforce)  │  │
│  └────────────────────────┘  │      │  └────────────────────────┘  │
└──────────────────────────────┘      └──────────────────────────────┘
```

---

## Deployment Options Comparison

| Feature | Local Dev | VPS/Cloud | Docker | Personal PC |
|---------|-----------|-----------|--------|-------------|
| **Best For** | Testing | Production | Containerized | Small teams |
| **Cost** | Free | $5-20/mo | $5-20/mo | Free |
| **Setup Time** | 15 min | 30 min | 20 min | 15 min |
| **Auto-Restart** | ❌ | ✅ (systemd) | ✅ | ❌/✅ (PM2) |
| **Persistence** | Local | Server disk | Volume | Local |
| **Public IP** | ❌ | Optional | Optional | ❌ |
| **SSL/HTTPS** | ❌ | Optional | Optional | ❌ |

---

## Data Flow

### Creating a Quote

```
1. User types in Slack:
   "/quote new for Acme Corp, Pro Plan, 50 seats"
                │
                ▼
2. Slack sends event via WebSocket
   to quotey-server
                │
                ▼
3. Agent Runtime processes intent:
   - Extracts: customer="Acme Corp", product="Pro Plan", seats=50
                │
                ▼
4. CPQ Engine validates:
   - Product exists? ✓
   - Pricing rules applied
   - Discounts calculated
                │
                ▼
5. Flow Engine manages state:
   - Creates Quote record
   - Tracks required/missing fields
                │
                ▼
6. Response sent back to Slack:
   - Quote number (Q-2026-0001)
   - Line items with pricing
   - Action buttons
                │
                ▼
7. SQLite stores:
   - Quote data
   - Audit trail
   - State transitions
```

### Quote Approval Flow

```
┌──────┐     ┌────────┐     ┌─────────┐     ┌──────────┐     ┌────────┐
│ Sales│────►│ Quote  │────►│ Pending │────►│ Manager  │────►│Approved│
│ User │     │ Created│     │ Approval│     │ Approves │     │        │
└──────┘     └────────┘     └─────────┘     └──────────┘     └────────┘
    │                                                │            │
    │                                                │            │
    ▼                                                ▼            ▼
┌──────────┐                                  ┌──────────────┐ ┌──────┐
│  Slack   │                                  │ Slack notify │ │ PDF  │
│ notification                                  │ + Email      │ │generated
└──────────┘                                  └──────────────┘ └──────┘
```

---

## Security Considerations

### Token Storage

```
❌ DON'T: Hardcode tokens in source code
❌ DON'T: Commit .env or config files with tokens
✅ DO: Use environment variables
✅ DO: Use a secrets manager in production
```

### Network Security

```
┌─────────────────────────────────────────┐
│           SLACK API                    │
│         (slack.com)                    │
└───────────────┬─────────────────────────┘
                │ WSS (WebSocket Secure)
                │ Outbound only
                ▼
┌─────────────────────────────────────────┐
│         Firewall (VPS)                 │
│  Inbound: 8080 (health) - Optional     │
│  Outbound: 443 (Slack API) - Required  │
└─────────────────────────────────────────┘
```

### File Permissions

```bash
# Config files should be readable only by owner
chmod 600 config/quotey.toml
chmod 600 .env

# Database should be readable/writable by service user
chmod 600 quotey.db
```

---

## Monitoring & Health Checks

### Health Check Endpoint

```bash
# Check if server is running
curl http://localhost:8080/health

# Response:
{
  "status": "healthy",
  "version": "0.1.2",
  "timestamp": "2026-03-06T10:30:00Z",
  "checks": {
    "database": "connected",
    "slack": "connected"
  }
}
```

### Log Monitoring

```bash
# View real-time logs (systemd)
sudo journalctl -u quotey -f

# View last 100 lines
sudo journalctl -u quotey -n 100

# Search for errors
sudo journalctl -u quotey | grep ERROR
```

### Key Metrics to Watch

| Metric | Command | Warning Sign |
|--------|---------|--------------|
| CPU Usage | `htop` or `top` | Consistently >80% |
| Memory Usage | `free -h` | Available <500MB |
| Disk Space | `df -h` | >80% full |
| Bot Response Time | Logs | >5 seconds |
| Database Size | `ls -lh *.db` | Growing rapidly |

---

## Backup Strategy

### Database Backup

```bash
# Automated daily backup script
#!/bin/bash
BACKUP_DIR="/backups/quotey"
DATE=$(date +%Y%m%d_%H%M%S)
DB_PATH="/var/lib/quotey/quotey.db"

# Create backup
sqlite3 "$DB_PATH" ".backup '$BACKUP_DIR/quotey_$DATE.db'"

# Compress
gzip "$BACKUP_DIR/quotey_$DATE.db"

# Keep only last 30 days
find "$BACKUP_DIR" -name "quotey_*.db.gz" -mtime +30 -delete
```

### Configuration Backup

```bash
# Backup config
cp /etc/quotey/quotey.toml /backups/quotey/config_$(date +%Y%m%d).toml

# Or use version control (without secrets!)
git add config/quotey.toml  # Only if tokens are env-interpolated
git commit -m "Backup config"
```

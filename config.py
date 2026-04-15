"""Configuration for WebhookCatch"""

import os
from pathlib import Path

# Base directory
BASE_DIR = Path(__file__).parent

# Database
DATABASE_PATH = BASE_DIR / "data" / "webhookcatch.db"

# Server
HOST = "0.0.0.0"
PORT = 5000

# Endpoint settings
ENDPOINT_ID_LENGTH = 8
DEFAULT_EXPIRY_HOURS = 24

# Request settings
MAX_BODY_SIZE = 1_000_000
STORE_BODY = True

# Auto-cleanup
AUTO_DELETE_HOURS = 168

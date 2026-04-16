"""Utility functions"""

import secrets
from typing import Dict, Optional
import string
from datetime import datetime, timedelta
from config import ENDPOINT_ID_LENGTH, DEFAULT_EXPIRY_HOURS

def generate_endpoint_id(length: int = ENDPOINT_ID_LENGTH) -> str:
    alphabet = string.ascii_lowercase + string.ascii_uppercase + string.digits
    alphabet = alphabet.replace('0', '').replace('O', '')
    alphabet = alphabet.replace('1', '').replace('l', '').replace('I', '')
    return ''.join(secrets.choice(alphabet) for _ in range(length))

def calculate_expiry(expires_in_hours: int = DEFAULT_EXPIRY_HOURS) -> datetime:
    return datetime.utcnow() + timedelta(hours=expires_in_hours)

def format_headers(headers) -> Dict[str, str]:
    hop_by_hop = {
        'connection', 'keep-alive', 'proxy-authenticate',
        'proxy-authorization', 'te', 'trailers',
        'transfer-encoding', 'upgrade', 'host'
    }
    return {
        k.decode() if isinstance(k, bytes) else k: 
        v.decode() if isinstance(v, bytes) else v
        for k, v in headers.items()
        if k.lower() not in hop_by_hop
    }

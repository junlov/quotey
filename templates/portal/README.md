# Quotey Web Portal Templates

This directory contains HTML templates for the Quotey customer-facing web portal. The portal allows customers to view quotes, approve/decline them, and communicate with sales reps.

## Templates

### `quote_viewer.html`
The main quote viewing page where customers can:
- View quote details, line items, and pricing
- Approve or decline quotes
- Download quote PDFs
- Post comments/questions
- See approval confirmation

**Features:**
- Responsive design (mobile-first)
- Accessible (WCAG 2.1 AA compliant)
- Toast notifications (no alerts)
- Loading states on buttons
- Modal dialogs for approval/decline
- Success state after approval
- Print styles
- Offline detection

**Data Model:**
```json
{
  "quote": {
    "quote_id": "Q-2026-0042",
    "token": "abc123",
    "status": "sent",
    "created_at": "Feb 26, 2026",
    "valid_until": "Mar 28, 2026",
    "term_months": 12,
    "subtotal": "$20,000.00",
    "discount_total": "$2,000.00",
    "tax_amount": "$0.00",
    "tax_rate": "0%",
    "total": "$18,000.00",
    "expires_soon": false,
    "lines": [
      {
        "product_name": "Pro Plan",
        "description": "Enterprise software license",
        "quantity": 150,
        "unit_price": "$10.00",
        "total": "$1,500.00",
        "sku": "PRO-001"
      }
    ]
  },
  "customer": {
    "name": "Acme Corp",
    "contact_name": "John Doe",
    "email": "john@acme.com",
    "phone": "+1 555-1234"
  },
  "rep": {
    "name": "Jane Smith",
    "email": "jane@example.com"
  },
  "branding": {
    "company_name": "Your Company",
    "logo_url": "https://example.com/logo.png",
    "primary_color": "#2563eb"
  },
  "comments": [
    {
      "author": "John Doe",
      "is_customer": true,
      "timestamp": "Feb 26, 2026",
      "text": "Can we get a discount for annual payment?"
    }
  ]
}
```

### `index.html`
The portal homepage showing all quotes for a customer:
- Statistics cards (pending, approved, total)
- Filter by status
- Sort by date/amount
- Search functionality
- Quote list table

**Features:**
- Real-time filtering (client-side)
- Sortable columns
- Responsive table
- Empty states

**Data Model:**
```json
{
  "customer": {
    "name": "Acme Corp",
    "email": "contact@acme.com"
  },
  "branding": { ... },
  "stats": {
    "pending": 3,
    "approved": 12,
    "total": 15
  },
  "quotes": [
    {
      "token": "abc123",
      "quote_id": "Q-2026-0042",
      "created_at": "Feb 26, 2026",
      "valid_until": "Mar 28, 2026",
      "status": "sent",
      "total_amount": "$18,000.00"
    }
  ]
}
```

## API Endpoints

The portal expects these API endpoints:

### POST `/quote/{token}/approve`
Approve a quote.
**Request:**
```json
{
  "approverName": "John Doe",
  "approverEmail": "john@acme.com",
  "comments": "Looks good!"
}
```
**Response:**
```json
{
  "success": true,
  "message": "Quote approved"
}
```

### POST `/quote/{token}/reject`
Decline a quote.
**Request:**
```json
{
  "reason": "Budget constraints"
}
```

### POST `/quote/{token}/comment`
Add a comment.
**Request:**
```json
{
  "text": "Can we discuss pricing?"
}
```

### GET `/quote/{token}/download`
Download quote PDF.

## Styling

### CSS Variables
All templates use CSS custom properties for easy theming:
```css
:root {
  --primary-color: #2563eb;
  --primary-hover: #1d4ed8;
  --success-color: #10b981;
  --danger-color: #ef4444;
  --bg-light: #f8fafc;
  --bg-white: #ffffff;
  --text-dark: #1e293b;
  --text-light: #64748b;
  --border-color: #e2e8f0;
}
```

### Responsive Breakpoints
- Desktop: > 768px
- Mobile: <= 768px

### Accessibility Features
- Focus indicators on all interactive elements
- ARIA labels and roles
- Semantic HTML
- Reduced motion support
- High contrast mode support

## Usage in Rust

```rust
use tera::{Context, Tera};

// Load templates
let tera = Tera::new("templates/**/*").unwrap();

// Render quote viewer
let mut context = Context::new();
context.insert("quote", &quote_data);
context.insert("customer", &customer_data);
context.insert("branding", &branding_data);

let html = tera.render("portal/quote_viewer.html", &context).unwrap();
```

## Security Considerations

1. **XSS Prevention**: All user input is escaped using `escapeHtml()` function
2. **CSRF Protection**: Implement CSRF tokens for POST requests
3. **Token Expiration**: Check token validity server-side
4. **Rate Limiting**: Implement rate limiting on API endpoints
5. **Input Validation**: Validate all form inputs server-side

## Browser Support

- Chrome/Edge (last 2 versions)
- Firefox (last 2 versions)
- Safari (last 2 versions)
- iOS Safari (last 2 versions)
- Chrome for Android (last 2 versions)

## Future Enhancements

- [ ] Dark mode support
- [ ] Real-time comment updates (WebSocket)
- [ ] E-signature capture canvas
- [ ] Quote comparison view
- [ ] Multi-language support

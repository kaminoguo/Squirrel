# Development Guide

This document provides detailed development setup and guidelines for Squirrel.

## Prerequisites

[TODO: Add required software and versions]
- Node.js / Python / etc
- Database (PostgreSQL / MongoDB / etc)
- Other dependencies

## Initial Setup

### 1. Clone Repository

```bash
git clone https://github.com/kaminoguo/Squirrel.git
cd Squirrel
```

### 2. Install Dependencies

```bash
[TODO: Add installation commands]
# npm install
# pip install -r requirements.txt
```

### 3. Environment Configuration

```bash
cp .env.example .env
# Edit .env with your local settings
```

### 4. Database Setup

```bash
[TODO: Add database setup commands]
# npm run db:migrate
# python manage.py migrate
```

### 5. Start Development Server

```bash
[TODO: Add dev server command]
# npm run dev
# python manage.py runserver
```

## Project Structure

```
[TODO: Add your project structure]
Squirrel/
├── src/
├── tests/
├── docs/
└── README.md
```

## Development Workflow

### Daily Routine

```bash
# Pull latest changes
git checkout main
git pull origin main

# Create feature branch
git checkout -b yourname/feature-name

# Work on your changes
# ...

# Run tests
[TODO: add test command]

# Commit and push
git add .
git commit -m "feat(scope): description"
git push origin yourname/feature-name

# Create PR on GitHub
```

## Testing

[TODO: Add testing instructions]

### Running Tests

```bash
# Run all tests
[test command]

# Run specific test
[test command for specific file]
```

## Code Style

[TODO: Add linting/formatting commands]

```bash
# Check code style
[lint command]

# Auto-format code
[format command]
```

## Common Tasks

[TODO: Add common development tasks]

## Troubleshooting

[TODO: Add common issues and solutions]

### Issue: [Problem]
**Solution:** [Solution]

## Additional Resources

- [.claude/CLAUDE.md](.claude/CLAUDE.md) - AI assistant context and standards
- [CONTRIBUTING.md](../CONTRIBUTING.md) - Quick contribution guide
- [README.md](../README.md) - Project overview

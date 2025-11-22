# Squirrel Project - Team Standards

## Team Communication Guidelines
1. DONT use unnecessary emojis that will affect our communication efficiency
2. READMEs and comments are for AI, not for humans; they should be written in a manner that facilitates AI comprehension
3. Always remain calm, do not seek quick success and instant benefits, and do not celebrate prematurely
4. Do not pander to ideas. If proposed solutions or concepts are incorrect or difficult to implement, point them out
5. Today is 2025 Nov23, if doing search tasks, search the latest
6. Do not display code when discussing solutions; it is a waste of time
7. All context in this project should be English, including commits, they should be brief English

## Git Workflow

### Branch Naming Convention
Format: `yourname/type-description`

Types:
- `feat` - New feature
- `fix` - Bug fix
- `refactor` - Code refactoring
- `docs` - Documentation
- `test` - Test additions/changes
- `chore` - Maintenance tasks

Examples:
- `lyrica/feat-add-authentication`
- `alice/fix-memory-leak`
- `bob/docs-update-api`

### Commit Message Format
Format: `type(scope): brief english description`

Keep commits brief and in English.

Examples:
- `feat(auth): add JWT validation`
- `fix(api): handle null user`
- `docs(readme): update setup`

### Pull Request Process
1. Create branch from `main`
2. Make changes and test
3. Push branch
4. Create PR on GitHub
5. Get 1 approval from teammate
6. Merge to main

## Development Standards

### Code Quality
- Write tests for new features
- Run linter before commit
- Keep files under 200 lines when possible
- Use descriptive names

### Security
- Never commit secrets (.env, API keys)
- Always validate user input
- Review AI-generated code for security issues

## Team Collaboration

All 3 team members are full-stack and can work on any part of the codebase.

### Communication
- Announce what you're working on in issues/PR
- If touching shared files, communicate with team
- Sync frequently: `git pull origin main` daily

### Conflict Prevention
- Pull latest before starting work
- Create focused branches for specific tasks
- Communicate when working on same areas 

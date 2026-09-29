# First login

Status: draft

Set the admin password with `--password`, an environment variable, the config file, or the first-access setup screen. If no password is set, wpg binds to localhost only and prints a warning. Sessions use HttpOnly, SameSite=Strict cookies and passwords are stored as argon2id hashes. After logging in, open the Dashboard to check interface status.

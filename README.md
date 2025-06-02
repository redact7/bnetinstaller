# bnetinstaller
very basic battle.net installer, only tested with black ops 4

#### usage
- make sure battle.net is open
- `.\bnetinstaller.exe --prod viper --lang enUS --dir "C:\Games\Black Ops 4"`

#### troubleshooting
- delete your log files in the folder `C:\ProgramData\Battle.net\Agent\Agent.9149\Logs` (a battle.net update might change that path, if so the Agent.XXXX folder will be a different name) then fully close and reopen battle.net
- make sure you have enough storage space to download the game, the installer wont warn you about this

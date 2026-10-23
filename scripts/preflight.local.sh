# Checks of eol.yml and public-text.yml (workflows the generator does not read).
# @quick
python3 scripts/test_check_eol.py
python3 scripts/check_eol.py
python3 scripts/test_public_text.py
scripts/public_text.sh

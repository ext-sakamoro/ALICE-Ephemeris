# Checks of eol.yml, public-text.yml and mutants-list.yml (workflows the generator does not read).
# @quick
python3 scripts/test_check_eol.py
python3 scripts/check_eol.py
python3 scripts/test_public_text.py
scripts/public_text.sh
python3 scripts/test_mutants_equivalent_check.py
python3 scripts/mutants_equivalent_check.py --list-only

"""Visible custom-select interactions shared by desktop browser fixtures."""
import json


def select_option(page, control, value):
    control.click()
    list_id = control.get_attribute("aria-controls")
    if not list_id:
        raise AssertionError("Select trigger does not identify a listbox")
    page.locator(
        f'[id={json.dumps(list_id)}] [role="option"][value={json.dumps(value)}]'
    ).click()

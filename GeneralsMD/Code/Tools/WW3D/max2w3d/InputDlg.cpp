// InputDlg.cpp : implementation file
//

#include "InputDlg.h"
#include <assert.h>


static BOOL CALLBACK _thunk_dialog_proc (HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam);


/////////////////////////////////////////////////////////////////////////////
// InputDlg dialog


InputDlg::InputDlg (HWND hWndParent)
:	m_hWndParent(hWndParent),
	m_hWnd(NULL)
{
	// Set the strings to default values.
	SetCaption("Input Value...");
	SetLabel("Please enter a value:");
	SetValue(NULL);
}


/////////////////////////////////////////////////////////////////////////////
// InputDlg Methods

int InputDlg::DoModal (void)
{
	// Put up the dialog box.
	BOOL result = DialogBoxParam(AppInstance, MAKEINTRESOURCE(IDD),
							m_hWndParent, (DLGPROC)_thunk_dialog_proc,
							(LPARAM)this);

	// Return IDOK if the user accepted the new settings.
	return (result == 1) ? IDOK : IDCANCEL;
}

void InputDlg::SetCaption (const char *caption)
{
	if (caption)
	{
		assert(strlen(caption) < sizeof(m_Caption));
		strcpy(m_Caption, caption);
	}
	else
		m_Caption[0] = '\0';
}

void InputDlg::SetLabel (const char *label)
{
	if (label)
	{
		assert(strlen(label) < sizeof(m_Label));
		strcpy(m_Label, label);
	}
	else
		m_Label[0] = '\0';
}

void InputDlg::SetValue (const char *value)
{
	if (value)
	{
		assert(strlen(value) < sizeof(m_Value));
		strcpy(m_Value, value);
	}
	else
		m_Value[0] = '\0';
}


/////////////////////////////////////////////////////////////////////////////
// InputDlg DialogProc

BOOL CALLBACK _thunk_dialog_proc (HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam)
{
	static InputDlg *dialog = NULL;

	if (uMsg == WM_INITDIALOG)
	{
		dialog = (InputDlg*)lParam;
		dialog->m_hWnd = hWnd;
	}

	if (dialog)
		return dialog->DialogProc(hWnd, uMsg, wParam, lParam);
	else
		return 0;
}

BOOL CALLBACK InputDlg::DialogProc (HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam)
{
	int code = HIWORD(wParam);

	switch (uMsg)
	{
		/*******************************************************************
		* WM_INITDIALOG
		*
		* Initialize all of the custom controls for the dialog box
		*
		*******************************************************************/
		case WM_INITDIALOG:

			OnInitDialog(wParam, lParam);
			return TRUE;


		/*******************************************************************
		* WM_COMMAND
		*
		*
		*******************************************************************/
		case WM_COMMAND:

			switch (LOWORD(wParam))
			{
				case IDOK:
					if (!OnOK())
						return TRUE;

					SetCursor(LoadCursor(NULL, IDC_WAIT));
					EndDialog(m_hWnd, 1);
					break;

				case IDCANCEL:
					EndDialog(m_hWnd, 0);
					break;
			}
			return TRUE;
	}

	return FALSE;
}

/////////////////////////////////////////////////////////////////////////////
// InputDlg message handlers

LRESULT InputDlg::OnInitDialog (WPARAM wParam, LPARAM lParam)
{
	// Set the cursor to the normal arrow.
	SetCursor(LoadCursor(NULL, IDC_ARROW));

	// Set the dialog box caption.
	SetWindowText(m_hWnd, m_Caption);

	// Set the label text.
	HWND hLabel = GetDlgItem(m_hWnd, IDC_LABEL);
	assert(hLabel != NULL);
	SetWindowText(hLabel, m_Label);

	// Set the default value.
	HWND hEdit = GetDlgItem(m_hWnd, IDC_VALUE);
	assert(hEdit != NULL);
	SetWindowText(hEdit, m_Value);

	// Select all of the text in the edit box.
	SendMessage(hEdit, EM_SETSEL, 0, -1);

	return 0;
}

BOOL InputDlg::OnOK (void)
{
	// Update our copy of what the user typed.
	HWND hEdit = GetDlgItem(m_hWnd, IDC_VALUE);
	assert(hEdit != NULL);
	GetWindowText(hEdit, m_Value, sizeof(m_Value));

	// The dialog can be dismissed.
	return TRUE;
}

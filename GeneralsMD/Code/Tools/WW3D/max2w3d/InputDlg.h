#ifndef INPUTDLG_H
#define INPUTDLG_H

#include "dllmain.h"
#include "resource.h"


/////////////////////////////////////////////////////////////////////////////
// InputDlg dialog - a generic input box for MAXScript

class InputDlg
{
	friend BOOL CALLBACK _thunk_dialog_proc (HWND, UINT, WPARAM, LPARAM);

public:

	// Construction
	InputDlg (HWND hWndParent=NULL);

	// Methods
	int DoModal (void);		// returns IDOK or IDCANCEL

	void SetCaption (const char *caption);
	void SetLabel (const char *label);
	void SetValue (const char *value);

	// DialogProc
	BOOL CALLBACK DialogProc (HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam);

	// Dialog data associated with GUI components.
	char	m_Value[1024];		// edit box
	char	m_Label[512];		// description label
	char	m_Caption[128];	// dialog caption

protected:

	// Dialog data
	enum			{ IDD = IDD_INPUT_DIALOG };
	HWND			m_hWnd;
	HWND			m_hWndParent;

	// Message Handlers
	LRESULT OnInitDialog (WPARAM wParam, LPARAM lParam);
	BOOL OnOK (void);
};


#endif

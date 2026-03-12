#ifndef EXPORTALLDLG_H
#define EXPORTALLDLG_H

#include "dllmain.h"
#include "resource.h"


class Interface;

/////////////////////////////////////////////////////////////////////////////
// ExportAllDlg dialog

class ExportAllDlg
{
public:

	// Construction
	ExportAllDlg (Interface *max_interface);

	// Methods
	int DoModal (void);

	// DialogProc
	BOOL CALLBACK DialogProc (HWND hWnd, UINT uMsg, WPARAM wParam, LPARAM lParam);

	// Dialog data associated with GUI components.
	char	m_Directory[_MAX_PATH];		// edit box
	BOOL	m_Recursive;					// check box

	// Dialog data
	enum			{ IDD = IDD_EXPORT_ALL };
	HWND			m_hWnd;
	Interface	*m_MaxInterface;

protected:

	// Message Handlers
	void OnInitDialog (void);
	void OnBrowse (void);
	BOOL OnOK (void);		// TRUE if ok to close dialog
};


#endif

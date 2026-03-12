// DebugWindow.h : main header file for the DEBUGWINDOW DLL
//

#if !defined(AFX_DEBUGWINDOW_H__018E1800_6E59_4527_BA0C_8731EBF22953__INCLUDED_)
#define AFX_DEBUGWINDOW_H__018E1800_6E59_4527_BA0C_8731EBF22953__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000

#ifndef __AFXWIN_H__
	#error include 'stdafx.h' before including this file for PCH
#endif

#include "resource.h"		// main symbols
#include "ParticleEditorExport.h"
/////////////////////////////////////////////////////////////////////////////
// CDebugWindowApp
// See DebugWindow.cpp for the implementation of this class
//

class DebugWindowDialog;

class CDebugWindowApp : public CWinApp
{
	public:
		CDebugWindowApp();
		~CDebugWindowApp();
		DebugWindowDialog* GetDialogWindow(void);
		void SetDialogWindow(DebugWindowDialog* pWnd);

	protected:
		DebugWindowDialog* m_DialogWindow;


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CDebugWindowApp)
	//}}AFX_VIRTUAL

	//{{AFX_MSG(CDebugWindowApp)
		// NOTE - the ClassWizard will add and remove member functions here.
		//    DO NOT EDIT what you see in these blocks of generated code !
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};


/////////////////////////////////////////////////////////////////////////////

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_DEBUGWINDOW_H__018E1800_6E59_4527_BA0C_8731EBF22953__INCLUDED_)

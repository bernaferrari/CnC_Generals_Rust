#if !defined(AFX_EULADIALOG_H__9F6A134E_C2A3_425E_8485_F90B1A5F9B58__INCLUDED_)
#define AFX_EULADIALOG_H__9F6A134E_C2A3_425E_8485_F90B1A5F9B58__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// euladialog.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// EulaDialog dialog

class EulaDialog : public CDialog
{
// Construction
public:
	EulaDialog(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(EulaDialog)
	enum { IDD = IDD_EULA_AGREEMENT };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(EulaDialog)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(EulaDialog)
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_EULADIALOG_H__9F6A134E_C2A3_425E_8485_F90B1A5F9B58__INCLUDED_)

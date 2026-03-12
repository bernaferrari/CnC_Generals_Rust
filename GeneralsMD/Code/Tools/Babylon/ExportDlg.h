#if !defined(AFX_EXPORTDLG_H__DDA81307_4F1A_11D3_B9DA_006097B90D93__INCLUDED_)
#define AFX_EXPORTDLG_H__DDA81307_4F1A_11D3_B9DA_006097B90D93__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// ExportDlg.h : header file
//

#include "expimp.h"

/////////////////////////////////////////////////////////////////////////////
// CExportDlg dialog

class CExportDlg : public CDialog
{
			LangID	langid;
			char		filename[200];
			TROPTIONS	options;
			int			got_lang;

// Construction
public:

	LangID			Language ( void )			{ return langid; };
	char*				Filename ( void )			{ return filename; };
	TROPTIONS*	Options ( void )			{ return &options; };

	CExportDlg(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(CExportDlg)
	enum { IDD = IDD_EXPORT };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CExportDlg)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(CExportDlg)
	virtual void OnOK();
	virtual void OnCancel();
	virtual BOOL OnInitDialog();
	afx_msg void OnSelchangeCombolang();
	afx_msg void OnSelendokCombolang();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};


//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_EXPORTDLG_H__DDA81307_4F1A_11D3_B9DA_006097B90D93__INCLUDED_)

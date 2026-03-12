// DlgProxy.h : header file
//

#if !defined(AFX_DLGPROXY_H__2BF3124F_3BA1_11D3_B9DA_006097B90D93__INCLUDED_)
#define AFX_DLGPROXY_H__2BF3124F_3BA1_11D3_B9DA_006097B90D93__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000

class CBabylonDlg;

/////////////////////////////////////////////////////////////////////////////
// CBabylonDlgAutoProxy command target

class CBabylonDlgAutoProxy : public CCmdTarget
{
	DECLARE_DYNCREATE(CBabylonDlgAutoProxy)

	CBabylonDlgAutoProxy();           // protected constructor used by dynamic creation

// Attributes
public:
	CBabylonDlg* m_pDialog;

// Operations
public:

// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(CBabylonDlgAutoProxy)
	public:
	virtual void OnFinalRelease();
	//}}AFX_VIRTUAL

// Implementation
protected:
	virtual ~CBabylonDlgAutoProxy();

	// Generated message map functions
	//{{AFX_MSG(CBabylonDlgAutoProxy)
		// NOTE - the ClassWizard will add and remove member functions here.
	//}}AFX_MSG

	DECLARE_MESSAGE_MAP()
	DECLARE_OLECREATE(CBabylonDlgAutoProxy)

	// Generated OLE dispatch map functions
	//{{AFX_DISPATCH(CBabylonDlgAutoProxy)
		// NOTE - the ClassWizard will add and remove member functions here.
	//}}AFX_DISPATCH
	DECLARE_DISPATCH_MAP()
	DECLARE_INTERFACE_MAP()
};

/////////////////////////////////////////////////////////////////////////////

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_DLGPROXY_H__2BF3124F_3BA1_11D3_B9DA_006097B90D93__INCLUDED_)

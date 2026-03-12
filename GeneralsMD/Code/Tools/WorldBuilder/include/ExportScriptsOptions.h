#if !defined(AFX_EXPORTSCRIPTSOPTIONS_H__706D8D87_E01C_431A_ADB8_DFC4CA8A8422__INCLUDED_)
#define AFX_EXPORTSCRIPTSOPTIONS_H__706D8D87_E01C_431A_ADB8_DFC4CA8A8422__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// ExportScriptsOptions.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// ExportScriptsOptions dialog

class ExportScriptsOptions : public CDialog
{
// Construction
public:
	ExportScriptsOptions(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(ExportScriptsOptions)
	enum { IDD = IDD_EXPORT_SCRIPTS_OPTIONS };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(ExportScriptsOptions)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:
	static Bool m_units;
	static Bool m_waypoints;
	static Bool m_triggers;
	static Bool m_allScripts;
	static Bool m_sides;

public:
	Bool getDoUnits(void) {return m_units;}
	Bool getDoWaypoints(void) {return m_waypoints;}
	Bool getDoTriggers(void) {return m_triggers;}
	Bool getDoAllScripts(void) {return m_allScripts;}
	Bool getDoSides(void) {return m_sides;}
	
protected:

	// Generated message map functions
	//{{AFX_MSG(ExportScriptsOptions)
	virtual void OnOK();
	virtual BOOL OnInitDialog();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_EXPORTSCRIPTSOPTIONS_H__706D8D87_E01C_431A_ADB8_DFC4CA8A8422__INCLUDED_)

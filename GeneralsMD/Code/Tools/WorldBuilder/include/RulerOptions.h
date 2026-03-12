
#ifndef __RULEROPTIONS_H_
#define __RULEROPTIONS_H_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// RulerOptions.h : header file
//
#include "OptionsPanel.h"

/////////////////////////////////////////////////////////////////////////////
// RulerOptions dialog

class RulerOptions : public COptionsPanel	
{

// Construction
public:

	RulerOptions(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(RulerOptions)
	enum { IDD = IDD_RULER_OPTIONS };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(RulerOptions)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual void OnOK(){return;};  //!< Modeless dialogs don't OK, so eat this for modeless.
	virtual void OnCancel(){return;}; //!< Modeless dialogs don't close on ESC, so eat this for modeless.
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(RulerOptions)
	virtual BOOL OnInitDialog();
	afx_msg void OnChangeWidthEdit();
	afx_msg void OnChangeCheckRuler();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()

protected:
	static RulerOptions *m_staticThis;  ///< Reference to the floating panel so SetWidth and SetFeather can be static.

	Bool		m_updating; ///<true if the ui is updating itself.

public:
	static void setWidth(Real width);
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif __RULEROPTIONS_H_

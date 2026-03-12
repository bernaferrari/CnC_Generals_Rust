#if !defined(AFX_LightOptions_H__6B56E20C_582E_4132_A251_879097C8852C__INCLUDED_)
#define AFX_LightOptions_H__6B56E20C_582E_4132_A251_879097C8852C__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// LightOptions.h : header file
//
#include "OptionsPanel.h"

class MapObject;
/////////////////////////////////////////////////////////////////////////////
// LightOptions dialog

class LightOptions : public COptionsPanel
{

// Construction
public:
	LightOptions(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(LightOptions)
	enum { IDD = IDD_LIGHT_OPTIONS };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(LightOptions)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual void OnOK(){return;};  //!< Modeless dialogs don't OK, so eat this for modeless.
	virtual void OnCancel(){return;}; //!< Modeless dialogs don't close on ESC, so eat this for modeless.
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(LightOptions)
	virtual BOOL OnInitDialog();
	afx_msg void OnChangeLightEdit();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()

protected:
	static LightOptions *m_staticThis;  ///< Reference to the floating panel so SetWidth and SetFeather can be static.
	Bool		m_updating; ///<true if the ui is updating itself.

protected:
	void updateTheUI(void);

public:
	static void update(void);
	static MapObject *getSingleSelectedLight(void);

};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_LightOptions_H__6B56E20C_582E_4030_A251_879097C8853C__INCLUDED_)

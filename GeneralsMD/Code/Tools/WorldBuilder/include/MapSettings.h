#if !defined(AFX_MAPSETTINGS_H__21749744_4DF4_462C_8DD4_FEEC1003DCFE__INCLUDED_)
#define AFX_MAPSETTINGS_H__21749744_4DF4_462C_8DD4_FEEC1003DCFE__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// MapSettings.h : header file
//

/////////////////////////////////////////////////////////////////////////////
// MapSettings dialog

class MapSettings : public CDialog
{
// Construction
public:
	MapSettings(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(MapSettings)
	enum { IDD = IDD_MAP_SETTINGS };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(MapSettings)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(MapSettings)
	afx_msg void OnChangeMapTimeofday();
	afx_msg void OnChangeMapWeather();
	virtual BOOL OnInitDialog();
	virtual void OnOK();
	afx_msg void OnChangeMapTitle();
	afx_msg void OnChangeMapCompression();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()
};

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_MAPSETTINGS_H__21749744_4DF4_462C_8DD4_FEEC1003DCFE__INCLUDED_)

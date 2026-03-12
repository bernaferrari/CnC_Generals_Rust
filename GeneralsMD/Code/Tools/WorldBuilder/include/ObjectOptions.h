#if !defined(AFX_ObjectOptions_H__D3FF66C5_7107_4DAC_8A29_5EBAB5C3A24E__INCLUDED_)
#define AFX_ObjectOptions_H__D3FF66C5_7107_4DAC_8A29_5EBAB5C3A24E__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// ObjectOptions.h : header file
//

#include "ObjectPreview.h"
#include "OptionsPanel.h"
#include "Common/AsciiString.h"
class WorldHeightMapEdit;
class MapObject;
/////////////////////////////////////////////////////////////////////////////
// ObjectOptions dialog

class ObjectOptions : public COptionsPanel
{
// Construction
public:
	ObjectOptions(CWnd* pParent = NULL);   ///< standard constructor

	~ObjectOptions(void);   ///< standard destructor
	enum { NAME_MAX_LEN = 64 };
// Dialog Data
	//{{AFX_DATA(ObjectOptions)
	enum { IDD = IDD_OBJECT_OPTIONS };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(ObjectOptions)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual void OnOK(){return;};  ///< Modeless dialogs don't OK, so eat this for modeless.
	virtual void OnCancel(){return;}; ///< Modeless dialogs don't close on ESC, so eat this for modeless.
	virtual BOOL OnNotify(WPARAM wParam, LPARAM lParam, LRESULT* pResult);
	//}}AFX_VIRTUAL

// Implementation
protected:

	// Generated message map functions
	//{{AFX_MSG(ObjectOptions)
	virtual BOOL OnInitDialog();
	afx_msg void OnEditchangeOwningteam();
	afx_msg void OnCloseupOwningteam();
	afx_msg void OnSelchangeOwningteam();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()


protected:
	static ObjectOptions *m_staticThis;
	static Bool				m_updating;
	static Int				m_currentObjectIndex;
	static char				m_currentObjectName[NAME_MAX_LEN];
	static AsciiString	m_curOwnerName;

	CTreeCtrl					m_objectTreeView;
	MapObject					*m_objectsList;
	ObjectPreview			m_objectPreview;

protected:
	void addObject( MapObject *mapObject, const char *pPath,  
									Int objectNdx, HTREEITEM parent );
	HTREEITEM findOrAdd(HTREEITEM parent, const char *pLabel);
	HTREEITEM findOrDont(const char *pLabel);
	HTREEITEM _FindOrDont(const char* pLabel, HTREEITEM startPoint);
	Bool setObjectTreeViewSelection(HTREEITEM parent, Int selection);
	void updateLabel();
	static MapObject *getCurMapObject(void);

public:
	static const char * getCurObjectName(void) {return m_currentObjectName;};
	static MapObject *duplicateCurMapObjectForPlace(const Coord3D* loc, Real angle, Bool checkPlayers = true);
	static MapObject *getObjectNamed(AsciiString name);
	static Int getObjectNamedIndex(const AsciiString& name);
	static void selectObject(const MapObject* pObj);
	static Real getCurObjectHeight(void);
	static void update();
	static AsciiString getCurGdfName(void);
}; 

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_ObjectOptions_H__D3FF66C5_711D_4DAC_8A29_5EAAB5C3A23E__INCLUDED_)

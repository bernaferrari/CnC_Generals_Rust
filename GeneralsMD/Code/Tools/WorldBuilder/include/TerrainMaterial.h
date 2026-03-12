#if !defined(AFX_TERRAINMATERIAL_H__D3FF66C5_711D_4DAC_8A29_5EAAB5C3A23E__INCLUDED_)
#define AFX_TERRAINMATERIAL_H__D3FF66C5_711D_4DAC_8A29_5EAAB5C3A23E__INCLUDED_

#if _MSC_VER > 1000
#pragma once
#endif // _MSC_VER > 1000
// terrainmaterial.h : header file
//

#include "WBPopupSlider.h"
#include "TerrainSwatches.h"
#include "OptionsPanel.h"
class WorldHeightMapEdit;
/////////////////////////////////////////////////////////////////////////////
// TerrainMaterial dialog

class TerrainMaterial : public COptionsPanel, public PopupSliderOwner
{
// Construction
public:
	TerrainMaterial(CWnd* pParent = NULL);   // standard constructor

// Dialog Data
	//{{AFX_DATA(TerrainMaterial)
	enum { IDD = IDD_TERRAIN_MATERIAL };
		// NOTE: the ClassWizard will add data members here
	//}}AFX_DATA


// Overrides
	// ClassWizard generated virtual function overrides
	//{{AFX_VIRTUAL(TerrainMaterial)
	protected:
	virtual void DoDataExchange(CDataExchange* pDX);    // DDX/DDV support
	virtual void OnOK(){return;};  ///< Modeless dialogs don't OK, so eat this for modeless.
	virtual void OnCancel(){return;}; ///< Modeless dialogs don't close on ESC, so eat this for modeless.
	virtual BOOL OnNotify(WPARAM wParam, LPARAM lParam, LRESULT* pResult);
	//}}AFX_VIRTUAL

// Implementation
protected:
	enum {MIN_TILE_SIZE=2, MAX_TILE_SIZE = 100};
	// Generated message map functions
	//{{AFX_MSG(TerrainMaterial)
	virtual BOOL OnInitDialog();
	afx_msg void OnSwapTextures();
	afx_msg void OnChangeSizeEdit();
	afx_msg void OnImpassable();
	afx_msg void OnPassableCheck();
	afx_msg void OnPassable();
	//}}AFX_MSG
	DECLARE_MESSAGE_MAP()


protected:
	static TerrainMaterial	*m_staticThis;
	Bool										m_updating;
	static Int							m_currentFgTexture;
	static Int							m_currentBgTexture;
	CTreeCtrl								m_terrainTreeView;
	TerrainSwatches					m_terrainSwatches;
	WBPopupSliderButton			m_widthPopup;
	Int											m_currentWidth;

	static Bool m_paintingPathingInfo;	 // If true, we are painting passable/impassable.  If false, normal texture painting.
	static Bool m_paintingPassable;

protected:
	void addTerrain(char *pPath, Int terrainNdx, HTREEITEM parent);
	HTREEITEM findOrAdd(HTREEITEM parent, char *pLabel);
	void updateLabel(void);

public:
	static Int getFgTexClass(void) {return m_currentFgTexture;}
	static Int getBgTexClass(void) {return m_currentBgTexture;}

	static void setFgTexClass(Int texClass);
	static void setBgTexClass(Int texClass);
	static void updateTextures(WorldHeightMapEdit *pMap);
	static void updateTextureSelection(void);
	static void setToolOptions(Bool singleCell);
	static void setWidth(Int width);

	static Bool isPaintingPathingInfo(void) {return m_paintingPathingInfo;}
	static Bool isPaintingPassable(void) {return m_paintingPassable;}

public:
	Bool setTerrainTreeViewSelection(HTREEITEM parent, Int selection);

	// Popup slider interface.
	virtual void GetPopSliderInfo(const long sliderID, long *pMin, long *pMax, long *pLineSize, long *pInitial);
	virtual void PopSliderChanged(const long sliderID, long theVal);
	virtual void PopSliderFinished(const long sliderID, long theVal);
}; 

//{{AFX_INSERT_LOCATION}}
// Microsoft Visual C++ will insert additional declarations immediately before the previous line.

#endif // !defined(AFX_TERRAINMATERIAL_H__D3FF66C5_711D_4DAC_8A29_5EAAB5C3A23E__INCLUDED_)

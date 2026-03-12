// WBFrameWnd.cpp : implementation file
//

#include "stdafx.h"
#include "worldbuilder.h"
#include "MainFrm.h"
#include "WBFrameWnd.h"
#include "WorldBuilderDoc.h"
#include "WHeightMapEdit.h"
#include "WbView3d.h"

/////////////////////////////////////////////////////////////////////////////
// CWBFrameWnd

IMPLEMENT_DYNCREATE(CWBFrameWnd, CFrameWnd)

CWBFrameWnd::CWBFrameWnd()
{
}

CWBFrameWnd::~CWBFrameWnd()
{
}

BOOL CWBFrameWnd::LoadFrame(UINT nIDResource,
				DWORD dwDefaultStyle,
				CWnd* pParentWnd,
				CCreateContext* pContext) {
	//dwDefaultStyle &= ~(WS_SIZEBOX|WS_MAXIMIZEBOX|WS_SYSMENU);

	BOOL ret = CFrameWnd::LoadFrame(nIDResource, dwDefaultStyle, CMainFrame::GetMainFrame(), pContext);
	if (ret) {
		Int top = ::AfxGetApp()->GetProfileInt(TWO_D_WINDOW_SECTION, "Top", 10);
		Int left =::AfxGetApp()->GetProfileInt(TWO_D_WINDOW_SECTION, "Left", 10);
		this->SetWindowPos(NULL, left,
			top, 0, 0,
			SWP_NOZORDER|SWP_NOSIZE);
		if (!m_cellSizeToolBar.Create(this, IDD_CELL_SLIDER, CBRS_LEFT, IDD_CELL_SLIDER))
		{
			DEBUG_CRASH(("Failed to create toolbar\n"));
		}
		EnableDocking(CBRS_ALIGN_ANY);
		m_cellSizeToolBar.SetupSlider();
		m_cellSizeToolBar.EnableDocking(CBRS_ALIGN_ANY);
		DockControlBar(&m_cellSizeToolBar);
	}
	return(ret);
}

void CWBFrameWnd::OnMove(int x, int y) 
{
	CFrameWnd::OnMove(x, y);
	if (this->IsWindowVisible() && !this->IsIconic()) {
		CRect frameRect;
		GetWindowRect(&frameRect);
		::AfxGetApp()->WriteProfileInt(TWO_D_WINDOW_SECTION, "Top", frameRect.top);
		::AfxGetApp()->WriteProfileInt(TWO_D_WINDOW_SECTION, "Left", frameRect.left);
	}
}


BEGIN_MESSAGE_MAP(CWBFrameWnd, CFrameWnd)
	//{{AFX_MSG_MAP(CWBFrameWnd)
	ON_WM_MOVE()
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CWBFrameWnd message handlers


/////////////////////////////////////////////////////////////////////////////
// CWB3dFrameWnd

IMPLEMENT_DYNCREATE(CWB3dFrameWnd, CMainFrame)

CWB3dFrameWnd::CWB3dFrameWnd()
{
}

CWB3dFrameWnd::~CWB3dFrameWnd()
{
}


BEGIN_MESSAGE_MAP(CWB3dFrameWnd, CMainFrame)
	//{{AFX_MSG_MAP(CWB3dFrameWnd)
		// NOTE - the ClassWizard will add and remove mapping macros here.
	ON_WM_MOVE()
	ON_COMMAND(ID_WINDOW_PREVIEW1024X768, OnWindowPreview1024x768)
	ON_UPDATE_COMMAND_UI(ID_WINDOW_PREVIEW1024X768, OnUpdateWindowPreview1024x768)
	ON_COMMAND(ID_WINDOW_PREVIEW640X480, OnWindowPreview640x480)
	ON_UPDATE_COMMAND_UI(ID_WINDOW_PREVIEW640X480, OnUpdateWindowPreview640x480)
	ON_COMMAND(ID_WINDOW_PREVIEW800X600, OnWindowPreview800x600)
	ON_UPDATE_COMMAND_UI(ID_WINDOW_PREVIEW800X600, OnUpdateWindowPreview800x600)
	//}}AFX_MSG_MAP
END_MESSAGE_MAP()

/////////////////////////////////////////////////////////////////////////////
// CWB3dFrameWnd message handlers
BOOL CWB3dFrameWnd::LoadFrame(UINT nIDResource,
				DWORD dwDefaultStyle,
				CWnd* pParentWnd,
				CCreateContext* pContext) {
	dwDefaultStyle &= ~(WS_SIZEBOX);

	BOOL ret = CMainFrame::LoadFrame(nIDResource, dwDefaultStyle, CMainFrame::GetMainFrame(), pContext);
	return(ret);
}


void CWB3dFrameWnd::OnMove(int x, int y) 
{
	CFrameWnd::OnMove(x, y);
	if (this->IsWindowVisible() && !this->IsIconic()) {
		CRect frameRect;
		GetWindowRect(&frameRect);
		::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Top", frameRect.top);
		::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Left", frameRect.left);
	}
}

void CWB3dFrameWnd::OnWindowPreview1024x768() 
{
	if (m_3dViewWidth == 1024) return;
	::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Width", 1024);
	::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Height", 768);
	adjustWindowSize();
}

void CWB3dFrameWnd::OnUpdateWindowPreview1024x768(CCmdUI* pCmdUI) 
{
	pCmdUI->SetCheck(m_3dViewWidth==1024?1:0);
}

void CWB3dFrameWnd::OnWindowPreview640x480() 
{
	if (m_3dViewWidth == 640) return;
	::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Width", 640);
	::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Height", 480);
	adjustWindowSize();
}

void CWB3dFrameWnd::OnUpdateWindowPreview640x480(CCmdUI* pCmdUI) 
{
	pCmdUI->SetCheck(m_3dViewWidth==640?1:0);
}

void CWB3dFrameWnd::OnWindowPreview800x600() 
{
	if (m_3dViewWidth == 800) return;
	::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Width", 800);
	::AfxGetApp()->WriteProfileInt(MAIN_FRAME_SECTION, "Height", 600);
	adjustWindowSize();
}

void CWB3dFrameWnd::OnUpdateWindowPreview800x600(CCmdUI* pCmdUI) 
{
	pCmdUI->SetCheck(m_3dViewWidth==800?1:0);
}

#pragma once


struct CParticleEditorPage : public CDialog
{
	UINT m_templateID;
	public:
		CParticleEditorPage(UINT nIDTemplate = 0, CWnd* pParentWnd = NULL);
		void InitPanel( int templateID );
		
		
	protected:
		afx_msg int OnCreate(LPCREATESTRUCT lpCreateStruct);
		DECLARE_MESSAGE_MAP()
};


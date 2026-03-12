
#pragma once

#ifndef __TEAMGENERIC_H__
#define __TEAMGENERIC_H__

class Dict;

class TeamGeneric : public CPropertyPage
{
	public:
		TeamGeneric();   // standard constructor

	// Dialog Data
		//{{AFX_DATA(TeamGeneric)
		enum { IDD = IDD_TeamGeneric };
		//}}AFX_DATA

		void setTeamDict(Dict *dict) { m_teamDict = dict; }
	protected:
		void _fillComboBoxesWithScripts();
		void _dictToScripts();
	
	protected:
		Dict *m_teamDict;
		

	protected: // Windows Functions
		virtual BOOL OnInitDialog();
		afx_msg void _scriptsToDict();
		afx_msg void OnScriptAdjust();
		DECLARE_MESSAGE_MAP()
};

#endif
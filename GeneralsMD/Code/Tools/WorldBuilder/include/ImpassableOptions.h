
#pragma once

#ifndef __IMPASSABLEOPTIONS_H__
#define __IMPASSABLEOPTIONS_H__

class ImpassableOptions : public CDialog
{
	public:
		enum { IDD = IDD_IMPASSABLEOPTIONS };

	public:
		ImpassableOptions(CWnd* pParent = NULL, Real defaultSlope = 45.0f);
		virtual ~ImpassableOptions();
		Real GetSlopeToShow() const { return m_slopeToShow; }
		Real GetDefaultSlope() const { return m_defaultSlopeToShow; }
		void SetDefaultSlopeToShow(Real slopeToShow) { m_slopeToShow = slopeToShow; }

	protected:
		Real m_slopeToShow;	// Clamped in the range of [0,90]
		Real m_defaultSlopeToShow;
		Bool m_showImpassableAreas;

		Bool ValidateSlope();	// Returns TRUE if it was valid, FALSE if it had to adjust it.
		
	protected:
		virtual BOOL OnInitDialog();
		afx_msg void OnAngleChange();
		afx_msg void OnPreview();
		DECLARE_MESSAGE_MAP()
};

#endif /* __IMPASSABLEOPTIONS_H__ */

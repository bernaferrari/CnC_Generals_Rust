//
// OLEString
//

#ifndef __OLESTRING_H
#define __OLESTRING_H

const unsigned int OLESTRING_DEFAULT_SIZE = 256;

class OLEString
{
	
	OLECHAR								*ole;
	char									*sb;
	unsigned int					len;
	int										locked;

	public:

	OLEString ( void ) ;
	~OLEString ();
	void		Set ( OLECHAR *new_ole );
	void		Set ( char *new_sb );
	OLECHAR*Get ( void ) { return ole; };
	int			Len ( void ) { return len; };
	char*		GetSB ( void ) { return sb; };
	void		StripSpaces ( void );
	void		FormatMetaString ( void );
	void		Lock ( void )	{ locked = TRUE; };
	void		Unlock ( void )	{ locked = FALSE; };
};

template <typename text> void StripSpaces ( text *string );
template <typename text> void ConvertMetaChars ( text *string );
template <typename text> void StripSpacesFromMetaString ( text *string );
template <typename text> int SameFormat ( text *string1, text *string2 );
template <typename text> void EncodeFormat ( text *string );
template <typename text> void DecodeFormat ( text *string );
template <typename text> int IsFormatTypeChar(  text string1 );



#endif // __OLESTRING_H
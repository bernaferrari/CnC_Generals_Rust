#ifndef __ENDIAN_H__
#define __ENDIAN_H__



/*
** Network order is big-endian.
**
** Packet router and mangler order is big or little endian depending on server platform.
**
** Game client order is little endian.
*/
extern bool BigEndian;

template<class T> inline T Endian(T val)
{
	if (!BigEndian) {
		return(val);
	}

/*
	char temp[sizeof(T)];
	*((T*)(&temp[0])) = val;
*/

	T retval = 0;

/*
	for (int i=0 ; i<sizeof(T) ; i++) {
		retval <<= 8;
		retval |= temp[i];
	}
*/

	int len = sizeof(T);
	unsigned char *c = (unsigned char *)(&val);
	for (int i=0; i<len; i++)
	{
		retval |= ( (*c++) << (8*i) );
	}

	return (retval);
}


#endif	//__ENDIAN_H__


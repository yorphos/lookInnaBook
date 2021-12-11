--
-- PostgreSQL database dump
--

-- Dumped from database version 13.4
-- Dumped by pg_dump version 13.4

SET statement_timeout = 0;
SET lock_timeout = 0;
SET idle_in_transaction_session_timeout = 0;
SET client_encoding = 'UTF8';
SET standard_conforming_strings = on;
SELECT pg_catalog.set_config('search_path', '', false);
SET check_function_bodies = false;
SET xmloption = content;
SET client_min_messages = warning;
SET row_security = off;

--
-- Name: base; Type: SCHEMA; Schema: -; Owner: steven
--

CREATE SCHEMA base;


ALTER SCHEMA base OWNER TO steven;

SET default_tablespace = '';

SET default_table_access_method = heap;

--
-- Name: address; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.address (
    address_id integer NOT NULL,
    street_address character varying(20),
    postal_code character varying(20),
    province character varying(10)
);


ALTER TABLE base.address OWNER TO steven;

--
-- Name: address_address_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.address ALTER COLUMN address_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.address_address_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: book; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.book (
    isbn integer NOT NULL,
    author_name character varying(20),
    genre character varying(20),
    publisher_id integer,
    num_pages integer,
    price numeric(6,2),
    author_royalties numeric(3,2),
    reorder_threshold integer,
    title character varying(50),
    stock integer,
    discontinued boolean
);


ALTER TABLE base.book OWNER TO steven;

--
-- Name: book_collection; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.book_collection (
    collection_id integer NOT NULL,
    curator_owner_id integer
);


ALTER TABLE base.book_collection OWNER TO steven;

--
-- Name: book_collection_collection_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.book_collection ALTER COLUMN collection_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.book_collection_collection_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: customer; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.customer (
    customer_id integer NOT NULL,
    name character varying(20),
    email character varying(20),
    password_hash character(60),
    default_shipping_address integer,
    default_payment_info_id integer
);


ALTER TABLE base.customer OWNER TO steven;

--
-- Name: customer_customer_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.customer ALTER COLUMN customer_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.customer_customer_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: in_cart; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.in_cart (
    isbn integer NOT NULL,
    customer_id integer NOT NULL,
    quantity integer
);


ALTER TABLE base.in_cart OWNER TO steven;

--
-- Name: in_collection; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.in_collection (
    collection_id integer NOT NULL,
    isbn integer NOT NULL
);


ALTER TABLE base.in_collection OWNER TO steven;

--
-- Name: in_order; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.in_order (
    isbn integer NOT NULL,
    order_id integer NOT NULL,
    quantity integer
);


ALTER TABLE base.in_order OWNER TO steven;

--
-- Name: orders; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.orders (
    order_id integer NOT NULL,
    customer_id integer,
    shipping_address_id integer,
    tracking_number character varying(30),
    order_status character varying(10),
    order_date date,
    payment_info_id integer
);


ALTER TABLE base.orders OWNER TO steven;

--
-- Name: orders_order_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.orders ALTER COLUMN order_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.orders_order_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: owner; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.owner (
    owner_id integer NOT NULL,
    name character varying(20),
    email character varying(30),
    password_hash character(60),
    password_salt bytea
);


ALTER TABLE base.owner OWNER TO steven;

--
-- Name: owner_owner_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.owner ALTER COLUMN owner_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.owner_owner_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: payment_info; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.payment_info (
    payment_info_id integer NOT NULL,
    name_on_card character varying(30),
    expiry character varying(10),
    card_number character varying(30),
    cvv character varying(10),
    billing_address_id integer
);


ALTER TABLE base.payment_info OWNER TO steven;

--
-- Name: payment_info_payment_info_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.payment_info ALTER COLUMN payment_info_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.payment_info_payment_info_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: publisher; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.publisher (
    publisher_id integer NOT NULL,
    company_name character varying(20),
    phone_number character varying(20),
    bank_number character varying(20),
    address_id integer,
    email character varying(30)
);


ALTER TABLE base.publisher OWNER TO steven;

--
-- Name: publisher_publisher_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.publisher ALTER COLUMN publisher_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.publisher_publisher_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: raw_sales_data; Type: VIEW; Schema: base; Owner: steven
--

CREATE VIEW base.raw_sales_data AS
 SELECT in_order.isbn,
    orders.order_id,
    orders.customer_id,
    orders.shipping_address_id,
    orders.tracking_number,
    orders.order_status,
    orders.order_date,
    orders.payment_info_id,
    in_order.quantity,
    book.author_name,
    book.genre,
    book.publisher_id,
    book.num_pages,
    book.price,
    book.author_royalties,
    book.reorder_threshold,
    book.title,
    book.stock,
    book.discontinued
   FROM ((base.orders
     JOIN base.in_order USING (order_id))
     JOIN base.book USING (isbn));


ALTER TABLE base.raw_sales_data OWNER TO steven;

--
-- Name: restock_order; Type: TABLE; Schema: base; Owner: steven
--

CREATE TABLE base.restock_order (
    restock_order_id integer NOT NULL,
    isbn integer,
    quantity integer,
    price_per_unit money,
    order_date date,
    order_status character varying(10)
);


ALTER TABLE base.restock_order OWNER TO steven;

--
-- Name: restock_order_restock_order_id_seq; Type: SEQUENCE; Schema: base; Owner: steven
--

ALTER TABLE base.restock_order ALTER COLUMN restock_order_id ADD GENERATED ALWAYS AS IDENTITY (
    SEQUENCE NAME base.restock_order_restock_order_id_seq
    START WITH 1
    INCREMENT BY 1
    NO MINVALUE
    NO MAXVALUE
    CACHE 1
);


--
-- Name: sales; Type: VIEW; Schema: base; Owner: steven
--

CREATE VIEW base.sales AS
 SELECT orders.order_date,
    sum(in_order.quantity) AS quantity
   FROM ((base.orders
     JOIN base.in_order USING (order_id))
     JOIN base.book USING (isbn))
  GROUP BY orders.order_date;


ALTER TABLE base.sales OWNER TO steven;

--
-- Data for Name: address; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.address (address_id, street_address, postal_code, province) FROM stdin;
1	12 Main St	K2W0A8	ON
2	777 U St.	K1S189	ON
3	12831 W	1231	IW
4	777 U St.	K1S189	ON
5	12831 W	1231	IW
6	777 U St.	K1S189	ON
7	12831 W	1231	IW
8	777 U St.	K1S189	ON
9	12831 W	1231	IW
10	777 U St.	K1S189	ON
11	12831 W	1231	IW
12	732 Main St	K192W9	ON
13	829 Recipe Avenue	FOOD	ON
\.


--
-- Data for Name: book; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.book (isbn, author_name, genre, publisher_id, num_pages, price, author_royalties, reorder_threshold, title, stock, discontinued) FROM stdin;
82381902	B. Kenny	Sci-Fi	2	300	29.99	0.10	10	Jarjar Journeys	29	f
82381967	B. BKenny	Sci-Fi	2	300	29.99	0.10	10	Jarjar Journeys 2	24	f
82381990	B. BKenny	Sci-Fi	2	300	29.99	0.10	10	Jarjar Journeys 3: Jarjar Redux Complete	28	f
312321	John Shefman	Cooking	4	25	50.00	0.10	5	Food For People Who Eat	7	f
\.


--
-- Data for Name: book_collection; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.book_collection (collection_id, curator_owner_id) FROM stdin;
\.


--
-- Data for Name: customer; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.customer (customer_id, name, email, password_hash, default_shipping_address, default_payment_info_id) FROM stdin;
2	Steve	test@local	$2b$10$BXeln024yC2PhT3J9jpuL.OYB9ciEUm0Lgvt3pDpTY0GlICWJGO4q	8	2
3	Steve	test2@local	$2b$10$27lLyoOAxeJBQlGbh0ZL0eFZ80VwquejFQD1ecygB3dbsTkokb9sS	10	3
\.


--
-- Data for Name: in_cart; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.in_cart (isbn, customer_id, quantity) FROM stdin;
\.


--
-- Data for Name: in_collection; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.in_collection (collection_id, isbn) FROM stdin;
\.


--
-- Data for Name: in_order; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.in_order (isbn, order_id, quantity) FROM stdin;
82381967	3	2
82381990	3	1
82381967	4	1
82381967	5	1
82381902	6	1
82381967	7	1
82381990	8	1
82381990	9	1
82381967	10	4
312321	11	3
\.


--
-- Data for Name: orders; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.orders (order_id, customer_id, shipping_address_id, tracking_number, order_status, order_date, payment_info_id) FROM stdin;
2	3	10	953146797	PR	2021-12-08	3
3	3	10	661630750	PR	2021-12-08	3
4	3	10	1290554229	PR	2021-12-08	3
5	3	10	3305752897	PR	2021-12-08	3
6	3	10	368815587	PR	2021-12-08	3
7	3	10	1887640785	PR	2021-12-08	3
8	3	10	1118788729	PR	2021-12-08	3
9	3	10	1910357174	PR	2021-12-09	3
10	3	10	2235082141	PR	2021-12-09	3
11	3	10	953908047	PR	2021-12-10	3
\.


--
-- Data for Name: owner; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.owner (owner_id, name, email, password_hash, password_salt) FROM stdin;
\.


--
-- Data for Name: payment_info; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.payment_info (payment_info_id, name_on_card, expiry, card_number, cvv, billing_address_id) FROM stdin;
1	Steven Pham	1/23	12312313	199	7
2	Steven Pham	1/23	12312313	199	9
3	Steven Pham	1/23	12312313	199	11
\.


--
-- Data for Name: publisher; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.publisher (publisher_id, company_name, phone_number, bank_number, address_id, email) FROM stdin;
2	ABC Books	888-999-0984	772823	1	publishing@abcbooks.com
3	CDE Books	613772222	77282890283901	12	publisher@cdebooks.com
4	Food Incorporated	2223334444	723819372193	13	publishing@foodcorp.com
\.


--
-- Data for Name: restock_order; Type: TABLE DATA; Schema: base; Owner: steven
--

COPY base.restock_order (restock_order_id, isbn, quantity, price_per_unit, order_date, order_status) FROM stdin;
\.


--
-- Name: address_address_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.address_address_id_seq', 13, true);


--
-- Name: book_collection_collection_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.book_collection_collection_id_seq', 1, false);


--
-- Name: customer_customer_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.customer_customer_id_seq', 3, true);


--
-- Name: orders_order_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.orders_order_id_seq', 11, true);


--
-- Name: owner_owner_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.owner_owner_id_seq', 1, false);


--
-- Name: payment_info_payment_info_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.payment_info_payment_info_id_seq', 3, true);


--
-- Name: publisher_publisher_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.publisher_publisher_id_seq', 4, true);


--
-- Name: restock_order_restock_order_id_seq; Type: SEQUENCE SET; Schema: base; Owner: steven
--

SELECT pg_catalog.setval('base.restock_order_restock_order_id_seq', 1, false);


--
-- Name: address address_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.address
    ADD CONSTRAINT address_pkey PRIMARY KEY (address_id);


--
-- Name: book_collection book_collection_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.book_collection
    ADD CONSTRAINT book_collection_pkey PRIMARY KEY (collection_id);


--
-- Name: book book_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.book
    ADD CONSTRAINT book_pkey PRIMARY KEY (isbn);


--
-- Name: customer customer_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.customer
    ADD CONSTRAINT customer_pkey PRIMARY KEY (customer_id);


--
-- Name: in_cart in_cart_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_cart
    ADD CONSTRAINT in_cart_pkey PRIMARY KEY (isbn, customer_id);


--
-- Name: in_collection in_collection_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_collection
    ADD CONSTRAINT in_collection_pkey PRIMARY KEY (collection_id, isbn);


--
-- Name: in_order in_order_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_order
    ADD CONSTRAINT in_order_pkey PRIMARY KEY (isbn, order_id);


--
-- Name: orders orders_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.orders
    ADD CONSTRAINT orders_pkey PRIMARY KEY (order_id);


--
-- Name: owner owner_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.owner
    ADD CONSTRAINT owner_pkey PRIMARY KEY (owner_id);


--
-- Name: payment_info payment_info_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.payment_info
    ADD CONSTRAINT payment_info_pkey PRIMARY KEY (payment_info_id);


--
-- Name: publisher publisher_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.publisher
    ADD CONSTRAINT publisher_pkey PRIMARY KEY (publisher_id);


--
-- Name: restock_order restock_order_pkey; Type: CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.restock_order
    ADD CONSTRAINT restock_order_pkey PRIMARY KEY (restock_order_id);


--
-- Name: book_collection book_collection_curator_owner_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.book_collection
    ADD CONSTRAINT book_collection_curator_owner_id_fkey FOREIGN KEY (curator_owner_id) REFERENCES base.owner(owner_id);


--
-- Name: book book_publisher_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.book
    ADD CONSTRAINT book_publisher_id_fkey FOREIGN KEY (publisher_id) REFERENCES base.publisher(publisher_id);


--
-- Name: customer customer_default_payment_info_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.customer
    ADD CONSTRAINT customer_default_payment_info_id_fkey FOREIGN KEY (default_payment_info_id) REFERENCES base.payment_info(payment_info_id);


--
-- Name: customer customer_default_shipping_address_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.customer
    ADD CONSTRAINT customer_default_shipping_address_fkey FOREIGN KEY (default_shipping_address) REFERENCES base.address(address_id);


--
-- Name: in_cart in_cart_customer_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_cart
    ADD CONSTRAINT in_cart_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES base.customer(customer_id);


--
-- Name: in_cart in_cart_isbn_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_cart
    ADD CONSTRAINT in_cart_isbn_fkey FOREIGN KEY (isbn) REFERENCES base.book(isbn);


--
-- Name: in_collection in_collection_collection_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_collection
    ADD CONSTRAINT in_collection_collection_id_fkey FOREIGN KEY (collection_id) REFERENCES base.book_collection(collection_id);


--
-- Name: in_collection in_collection_isbn_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_collection
    ADD CONSTRAINT in_collection_isbn_fkey FOREIGN KEY (isbn) REFERENCES base.book(isbn);


--
-- Name: in_order in_order_isbn_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_order
    ADD CONSTRAINT in_order_isbn_fkey FOREIGN KEY (isbn) REFERENCES base.book(isbn);


--
-- Name: in_order in_order_order_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.in_order
    ADD CONSTRAINT in_order_order_id_fkey FOREIGN KEY (order_id) REFERENCES base.orders(order_id);


--
-- Name: orders orders_customer_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.orders
    ADD CONSTRAINT orders_customer_id_fkey FOREIGN KEY (customer_id) REFERENCES base.customer(customer_id);


--
-- Name: orders orders_payment_info_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.orders
    ADD CONSTRAINT orders_payment_info_id_fkey FOREIGN KEY (payment_info_id) REFERENCES base.payment_info(payment_info_id);


--
-- Name: payment_info payment_info_billing_address_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.payment_info
    ADD CONSTRAINT payment_info_billing_address_id_fkey FOREIGN KEY (billing_address_id) REFERENCES base.address(address_id);


--
-- Name: publisher publisher_address_id_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.publisher
    ADD CONSTRAINT publisher_address_id_fkey FOREIGN KEY (address_id) REFERENCES base.address(address_id);


--
-- Name: restock_order restock_order_isbn_fkey; Type: FK CONSTRAINT; Schema: base; Owner: steven
--

ALTER TABLE ONLY base.restock_order
    ADD CONSTRAINT restock_order_isbn_fkey FOREIGN KEY (isbn) REFERENCES base.book(isbn);


--
-- PostgreSQL database dump complete
--


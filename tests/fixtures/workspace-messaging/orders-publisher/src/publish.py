def publish_order(producer):
    producer.send("orders.created", value=b"order-1")
